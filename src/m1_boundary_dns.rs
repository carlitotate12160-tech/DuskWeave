//! ADR-015/ADR-017 live boundary effect adapter: spawns the qualified
//! dw-boundary-launcher for one issued DNS observation and maps its receipt
//! back onto the issued request. No network access happens in this process.
use crate::m1_terrain_input::{
    DnsAnswer, DnsCompleteness, DnsRcode, TerrainDnsRequest, TerrainRequestVersion,
    TerrainSourceKind,
};
use crate::{Fail, Res};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

const ATTEMPT_DEADLINE_MS: i64 = 30000;
const RECEIPT_WAIT_MS: u64 = 35000;

pub fn acquire(mut issued: TerrainDnsRequest) -> Res<TerrainDnsRequest> {
    if (issued.version, issued.source_kind)
        != (
            TerrainRequestVersion::M1TerrainDnsIssuedObservationV1,
            TerrainSourceKind::BoundaryDnsAcquisition,
        )
    {
        return Err(Fail::State("boundary_source_refused"));
    }
    issued.validate()?;
    let receipt = drive(&issued)?;
    apply(&mut issued, &receipt)?;
    issued.validate()?;
    Ok(issued)
}

fn request(issued: &TerrainDnsRequest, deadline_ms: i64) -> Value {
    json!({
        "version": "dw_boundary_attempt_v1",
        "session_id": issued.correlation_id.simple().to_string(),
        "attempt_id": issued.correlation_id.simple().to_string(),
        "control_root": env("DW_M1_RUN_CONTROL_ROOT"),
        "image": env("DW_BOUNDARY_IMAGE"),
        "effect": "dns_udp",
        "qname": issued.subject_name,
        "qtype": "A",
        "endpoint": {"ipv4": issued.resolver_ipv4, "port": 53, "transport": "udp"},
        "txid": txid(issued.correlation_id),
        "deadline_ms": deadline_ms,
        "max_wire": 512,
        "max_raw": 2048,
        "lease_ms": 1500_u64,
        "renew_ms": 400_u64,
    })
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

fn txid(id: uuid::Uuid) -> u16 {
    let b = id.as_bytes();
    u16::from_be_bytes([b[0], b[1]])
}

fn drive(issued: &TerrainDnsRequest) -> Res<Value> {
    let launcher = std::env::var("DW_BOUNDARY_LAUNCHER")
        .map_err(|_| Fail::State("boundary_launcher_unavailable"))?;
    let image = env("DW_BOUNDARY_IMAGE");
    if !(image.contains("@sha256:") || image.starts_with("sha256:")) {
        return Err(Fail::State("boundary_image_unpinned"));
    }
    let deadline_ms = now_ms()? + ATTEMPT_DEADLINE_MS;
    let mut child = Command::new(launcher)
        .arg("run-attempt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Fail::State("boundary_launcher_unavailable"))?;
    let result = exchange(&mut child, &request(issued, deadline_ms));
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn now_ms() -> Res<i64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .map_err(|_| Fail::State("boundary_clock"))
}

fn exchange(child: &mut Child, req: &Value) -> Res<Value> {
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or(Fail::State("boundary_pipe_lost"))?;
        let framed = wire_frame(req)?;
        stdin
            .write_all(&framed)
            .and_then(|()| stdin.flush())
            .map_err(|_| Fail::State("boundary_request_failed"))?;
    }
    let stdout = child
        .stdout
        .take()
        .ok_or(Fail::State("boundary_pipe_lost"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(RECEIPT_WAIT_MS);
    for line in BufReader::new(stdout).lines() {
        if std::time::Instant::now() > deadline {
            return Err(Fail::State("boundary_receipt_timeout"));
        }
        let Ok(line) = line else {
            return Err(Fail::State("boundary_pipe_lost"));
        };
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if v.get("version").and_then(Value::as_str) == Some("dw_boundary_receipt_v1")
            && v.get("kind").and_then(Value::as_str) == Some("attempt")
        {
            return Ok(v);
        }
    }
    Err(Fail::State("boundary_receipt_missing"))
}

fn wire_frame(req: &Value) -> Res<Vec<u8>> {
    let body = serde_json::to_vec(req).map_err(|_| Fail::State("boundary_encode"))?;
    let mut framed = Vec::with_capacity(body.len() + 4);
    framed.extend_from_slice(&(body.len() as u32).to_be_bytes());
    framed.extend_from_slice(&body);
    Ok(framed)
}

fn apply(issued: &mut TerrainDnsRequest, receipt: &Value) -> Res<()> {
    let wrong = [
        receipt["attempt_id"].as_str() != Some(issued.correlation_id.simple().to_string().as_str()),
        receipt["session_id"].as_str() != Some(issued.correlation_id.simple().to_string().as_str()),
    ];
    if wrong.into_iter().any(|b| b) {
        return Err(Fail::State("boundary_receipt_binding"));
    }
    let disposition = receipt["disposition"].as_str().unwrap_or("internal_fault");
    match disposition {
        "qualified" | "nxdomain" => fill(issued, receipt)?,
        _ => return Err(Fail::State("boundary_effect_failed")),
    }
    issued.perceived_at = now_seconds()?;
    Ok(())
}

fn fill(issued: &mut TerrainDnsRequest, receipt: &Value) -> Res<()> {
    let rcode = receipt["rcode"].as_str().unwrap_or("servfail");
    issued.rcode = serde_json::from_value::<DnsRcode>(json!(rcode))
        .map_err(|_| Fail::State("boundary_receipt_decode"))?;
    issued.truncated = receipt["truncated"].as_bool().unwrap_or(false);
    issued.completeness = DnsCompleteness::Complete;
    issued.answers = parse_answers(receipt)?;
    Ok(())
}

fn parse_answers(receipt: &Value) -> Res<Vec<DnsAnswer>> {
    let empty = Vec::new();
    let raw = receipt["answers"].as_array().unwrap_or(&empty);
    let mut answers = Vec::with_capacity(raw.len());
    for a in raw {
        let (ipv4, ttl) = (a["ipv4"].as_str(), a["ttl"].as_u64());
        let (Some(ipv4), Some(ttl)) = (ipv4, ttl) else {
            return Err(Fail::State("boundary_receipt_decode"));
        };
        answers.push(DnsAnswer {
            ipv4: ipv4.into(),
            ttl_seconds: ttl as u32,
        });
    }
    Ok(answers)
}

fn now_seconds() -> Res<i64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .map_err(|_| Fail::State("boundary_clock"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::m1_terrain_input::{DnsLimitation, DnsQuestionClass, DnsQuestionType, DnsTransport};
    use crate::mission::{AssetRef, CampaignId, EngagementId, GoalRef, OperatorRef};
    use uuid::Uuid;

    fn u() -> Uuid {
        Uuid::from_bytes([7; 16])
    }

    fn issued() -> TerrainDnsRequest {
        TerrainDnsRequest {
            version: TerrainRequestVersion::M1TerrainDnsIssuedObservationV1,
            engagement_id: EngagementId(u()),
            campaign_id: CampaignId(u()),
            operator_ref: OperatorRef(u()),
            expected_mission_revision: 1,
            goal_ref: GoalRef(u()),
            exercise_mode: crate::mission::ExerciseMode::Blind,
            correlation_id: u(),
            source_kind: TerrainSourceKind::BoundaryDnsAcquisition,
            vantage_ref: u(),
            resolver_ipv4: "1.1.1.1".into(),
            subject_name: "niagamas.com".into(),
            asset_ref: Some(AssetRef(u())),
            expected_claim_revision: 0,
            perceived_at: 1,
            uncertainty_seconds: 0,
            transport: DnsTransport::Udp,
            qtype: DnsQuestionType::A,
            qclass: DnsQuestionClass::In,
            rcode: DnsRcode::Noerror,
            truncated: false,
            completeness: DnsCompleteness::Complete,
            limitations: vec![DnsLimitation::SingleVantageOnly],
            answers: vec![],
        }
    }

    fn receipt(disposition: &str) -> Value {
        json!({
            "version": "dw_boundary_receipt_v1",
            "kind": "attempt",
            "session_id": u().simple().to_string(),
            "attempt_id": u().simple().to_string(),
            "disposition": disposition,
            "rcode": "noerror",
            "truncated": false,
            "answers": [{"ipv4": "206.189.93.100", "ttl": 1799}],
            "counters": {"raw_bytes": 56, "raw_truncated": false},
            "worker_elapsed_ms": 464,
        })
    }

    #[test]
    fn qualified_receipt_fills_answers() {
        let mut q = issued();
        apply(&mut q, &receipt("qualified")).expect("apply");
        assert_eq!(q.answers.len(), 1);
        assert_eq!(q.answers[0].ipv4, "206.189.93.100");
        assert_eq!(q.answers[0].ttl_seconds, 1799);
        assert!(q.perceived_at > 1);
    }

    #[test]
    fn nxdomain_maps_rcode_empty_answers() {
        let mut q = issued();
        let mut r = receipt("nxdomain");
        r["rcode"] = json!("nxdomain");
        r["answers"] = json!([]);
        apply(&mut q, &r).expect("apply");
        assert_eq!(q.rcode, DnsRcode::Nxdomain);
        assert!(q.answers.is_empty());
    }

    #[test]
    fn failed_dispositions_refuse() {
        for d in [
            "timeout",
            "stopped",
            "no_data",
            "malformed",
            "request_refused",
        ] {
            let mut q = issued();
            assert!(apply(&mut q, &receipt(d)).is_err(), "{d}");
        }
    }

    #[test]
    fn forged_attempt_binding_refuses() {
        let mut q = issued();
        let mut r = receipt("qualified");
        r["attempt_id"] = json!("deadbeef");
        assert!(apply(&mut q, &r).is_err());
    }

    #[test]
    fn fixture_source_refused() {
        let mut q = issued();
        q.source_kind = TerrainSourceKind::FixtureDnsAcquisition;
        assert!(acquire(q).is_err());
    }

    #[test]
    fn request_carries_bound_fields() {
        let r = request(&issued(), 1793000000000);
        assert_eq!(r["endpoint"]["ipv4"], json!("1.1.1.1"));
        assert_eq!(r["qname"], json!("niagamas.com"));
        assert_eq!(r["endpoint"]["port"], json!(53));
        assert_eq!(r["version"], json!("dw_boundary_attempt_v1"));
        assert_eq!(r["deadline_ms"], json!(1793000000000_i64));
    }
}
