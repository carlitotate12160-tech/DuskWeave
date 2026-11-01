use duskweave::m1_dns::{DnsResult, FrameReader};
use serde_json::{Value, json};
use std::io::Cursor;

fn result() -> Value {
    json!({"schema":"m1_dns_ipc_v1","kind":"RESULT",
        "attempt":"00000000-0000-0000-0000-000000000001","session_generation":1,
        "sequence":3,"status":"completed","observed_at_ms":1000,"rcode":0,
        "truncated":false,"complete":true,"withheld_aliases":0,
        "answers":[{"ipv4":"198.51.100.7","ttl_seconds":30}],"aliases":[]})
}
fn frame(raw: &[u8]) -> Vec<u8> {
    let mut framed = (raw.len() as u32).to_be_bytes().to_vec();
    framed.extend_from_slice(raw);
    framed
}
fn decode(raw: &[u8]) -> Result<DnsResult, duskweave::Fail> {
    FrameReader::new(Cursor::new(frame(raw))).result()
}

#[test]
fn trusted_result_codec_rejects_noncanonical_shapes_and_raw_fields() {
    let valid = result();
    assert!(decode(&serde_json::to_vec(&valid).unwrap()).is_ok());
    for field in [
        "schema", "attempt", "status", "complete", "answers", "aliases",
    ] {
        let mut bad = valid.clone();
        bad.as_object_mut().unwrap().remove(field);
        assert!(
            decode(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "required {field}"
        );
    }
    for (field, value) in [
        ("packet", json!("RAW_SENTINEL")),
        ("status", json!("arbitrary_detail")),
        ("sequence", json!(4294967296u64)),
        ("answers", json!([["198.51.100.7", 30]])),
        (
            "answers",
            json!([{"ipv4":"198.51.100.7","ttl_seconds":30,"detail":"RAW_SENTINEL"}]),
        ),
        (
            "answers",
            json!(vec![json!({"ipv4":"198.51.100.7","ttl_seconds":30}); 65]),
        ),
    ] {
        let mut bad = valid.clone();
        bad[field] = value;
        assert!(
            decode(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "closed {field}"
        );
    }
    let text = serde_json::to_string(&valid).unwrap();
    let duplicate = text.replacen('{', "{\"sequence\":3,", 1);
    assert!(decode(duplicate.as_bytes()).is_err());
    assert!(decode(format!("{text}{text}").as_bytes()).is_err());
    assert!(decode(b"null").is_err());
}

#[test]
fn frame_lengths_and_partial_input_fail_without_allocating_claimed_capacity() {
    for bytes in [
        vec![],
        vec![0, 0, 0],
        vec![255; 4],
        vec![0, 0, 64, 1],
        vec![0, 0, 0, 2, b'{'],
    ] {
        assert!(FrameReader::new(Cursor::new(bytes)).result().is_err());
    }
}

#[test]
fn stop_only_host_commands_have_closed_correlated_wire_contracts() {
    use duskweave::m1_effect_control::{HostChannel, HostCommand};
    for raw in [
        br#"{"kind":"StopRun","run_id":"00000000-0000-0000-0000-000000000001"}"#.as_slice(),
        br#"{"kind":"StopCampaign","engagement_id":"00000000-0000-0000-0000-000000000001","campaign_id":"00000000-0000-0000-0000-000000000002"}"#.as_slice(),
    ] {
        let command = HostChannel::new(Cursor::new(frame(raw))).command();
        assert!(command.is_ok(), "authenticated stop must not require acquisition data");
        let encoded = serde_json::to_vec(&command.unwrap()).unwrap();
        let mut extra: Value = serde_json::from_slice(&encoded).unwrap();
        extra["path"] = json!("/caller-controlled");
        let mut channel = HostChannel::new(Cursor::new(frame(&serde_json::to_vec(&extra).unwrap())));
        assert!(channel.command().is_err(), "stop commands admit no executable or path");
        let duplicate = String::from_utf8(encoded).unwrap().replacen('{', "{\"kind\":\"StopRun\",", 1);
        assert!(HostChannel::new(Cursor::new(frame(duplicate.as_bytes()))).command().is_err());
    }
    let _ = std::mem::size_of::<HostCommand>();
}
