use duskweave::m1_reasoning::{
    proposal::ProposalBasis, provider_diagnostic::ProviderDiagnostic, response,
};
use serde_json::json;
use std::time::Duration;
struct Basis;
impl ProposalBasis for Basis {
    fn reference_eligible(&self, _: &str) -> bool {
        false
    }
    fn manifest_eligible(&self, _: &str) -> bool {
        false
    }
    fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    fn safe_explanation(&self, _: &str) -> bool {
        true
    }
}
#[test]
fn incomplete_http_retains_safe_report() {
    let envelope = json!({"status":"incomplete", "incomplete_details":{"reason":"max_output_tokens"},
        "usage":{"input_tokens":12,"output_tokens":23,"output_tokens_details":{"reasoning_tokens":20}},
        "error":{"message":"SENTINEL_ERROR"},"output":[{"type":"reasoning","text":"SENTINEL_PRIVATE"}]});
    let raw = http_envelope(&envelope);
    let mut diagnostic = ProviderDiagnostic::new("binding-fixture".into());
    assert!(matches!(
        response::admit_http(&raw, Duration::ZERO, &mut Basis, &mut diagnostic),
        Err("provider_incomplete")
    ));
    let value = serde_json::to_value(diagnostic).unwrap();
    assert_eq!(
        value["report"]["provider_response"],
        json!({"status":"incomplete","incomplete_reason":"max_output_tokens","usage_availability":"reported"})
    );
    assert_eq!(value["report"]["input_tokens"], 12);
    assert_eq!(value["report"]["reasoning_tokens"], 20);
    assert!(!value.to_string().contains("SENTINEL"));
}
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_native_inference.rs"]
mod native;
use duskweave::m1_reasoning::{inference::*, request};
use serde_json::Value;
use std::time::Instant;
use uuid::Uuid;
fn http_envelope(value: &Value) -> Vec<u8> {
    format!("HTTP/1.1 200 OK\r\n\r\n{value}").into_bytes()
}
fn decode(value: Value) -> ProviderDiagnostic {
    let mut d = ProviderDiagnostic::default();
    let raw = http_envelope(&value);
    let _ = response::admit_http(&raw, Duration::ZERO, &mut Basis, &mut d);
    d
}
fn incomplete() -> Value {
    json!({"status":"incomplete","model":"deepseek-flash",
        "incomplete_details":{"reason":"max_output_tokens","private":"SENTINEL"},
        "usage":{"input_tokens":12,"output_tokens":23,"output_tokens_details":{"reasoning_tokens":20}},
        "error":{"message":"SENTINEL"},"id":"SENTINEL",
        "output":[{"type":"reasoning","text":"SENTINEL"}]})
}
#[test]
fn closed_status_reason_and_individual_usage_projection() {
    for (status, normalized) in [
        (json!("completed"), "completed"),
        (json!("incomplete"), "incomplete"),
        (json!("failed"), "failed"),
        (json!("in_progress"), "in_progress"),
        (json!("SENTINEL"), "unrecognized"),
        (json!(17), "unrecognized"),
        (Value::Null, "missing"),
    ] {
        let mut v = incomplete();
        v["status"] = status;
        let safe = serde_json::to_value(decode(v)).unwrap();
        assert_eq!(safe["report"]["provider_response"]["status"], normalized);
        if normalized != "incomplete" {
            assert!(safe["report"]["provider_response"]["incomplete_reason"].is_null());
        }
        assert!(!safe.to_string().contains("SENTINEL"));
    }
    for (reason, normalized) in [
        (json!("content_filter"), "content_filter"),
        (json!("SENTINEL"), "unknown"),
        (Value::Null, "unknown"),
    ] {
        let mut v = incomplete();
        v["incomplete_details"]["reason"] = reason;
        assert_eq!(
            serde_json::to_value(decode(v)).unwrap()["report"]["provider_response"]["incomplete_reason"],
            normalized
        );
    }
    let mut missing = incomplete();
    missing.as_object_mut().unwrap().remove("status");
    assert_eq!(
        serde_json::to_value(decode(missing)).unwrap()["report"]["provider_response"]["status"],
        "missing"
    );
    for (usage, availability, input, output) in [
        (Value::Null, "unavailable", Value::Null, Value::Null),
        (json!({"input_tokens":0}), "partial", json!(0), Value::Null),
        (
            json!({"input_tokens":12,"output_tokens":-1}),
            "partial",
            json!(12),
            Value::Null,
        ),
        (
            json!({"input_tokens":"12","output_tokens":1.5}),
            "unavailable",
            Value::Null,
            Value::Null,
        ),
        (
            json!({"input_tokens":9223372036854775808u64,"output_tokens":7}),
            "partial",
            Value::Null,
            json!(7),
        ),
    ] {
        let mut v = incomplete();
        v["usage"] = usage;
        let safe = serde_json::to_value(decode(v)).unwrap();
        assert_eq!(
            safe["report"]["provider_response"]["usage_availability"],
            availability
        );
        assert_eq!(safe["report"]["input_tokens"], input);
        assert_eq!(safe["report"]["output_tokens"], output);
    }
}
#[test]
fn truncated_and_oversized_envelopes_clear_report() {
    let mut diagnostic = decode(incomplete());
    for raw in [
        b"HTTP/1.1 200 OK\r\n\r\n{\"status\":\"incomplete\"".to_vec(),
        vec![b'x'; 65537],
    ] {
        assert!(response::admit_http(&raw, Duration::ZERO, &mut Basis, &mut diagnostic).is_err());
        assert!(diagnostic.report.is_none());
    }
}
// Synthetic HTTP only: this fixture uses the production decoder, never a live provider.
struct EnvelopeProvider {
    diagnostic: ProviderDiagnostic,
    sends: usize,
    accounting_backend: Option<i32>,
    wire: Vec<u8>,
}
impl EnvelopeProvider {
    fn new(accounting_backend: Option<i32>) -> Self {
        Self {
            diagnostic: ProviderDiagnostic::default(),
            sends: 0,
            accounting_backend,
            wire: http_envelope(&incomplete()),
        }
    }
}
impl Provider for EnvelopeProvider {
    fn terminal_report(&self, digest: &str) -> Option<&ProviderReport> {
        if self.diagnostic.binding_digest == digest {
            self.diagnostic.report.as_ref()
        } else {
            None
        }
    }
    fn prepare(&mut self, b: &request::BoundCall, _: Instant) -> Result<(), &'static str> {
        self.diagnostic = ProviderDiagnostic::new(b.digest.clone());
        Ok(())
    }
    fn call(
        &mut self,
        _: &request::BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        release()?;
        self.sends += 1;
        if let Some(pid) = self.accounting_backend {
            assert!(
                native::client("DW_INFERENCE_BROKER_DATABASE_URL")
                    .query_one("SELECT pg_terminate_backend($1)", &[&pid])
                    .unwrap()
                    .get::<_, bool>(0)
            );
        }
        response::admit_http(&self.wire, Duration::ZERO, basis, &mut self.diagnostic)
    }
}
fn cli(command: &str, path: &std::path::Path, decision: Uuid) -> Value {
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_duskweave"));
    child
        .env_clear()
        .env("DW_DATABASE_CONFIG_MODE", "env-local")
        .env(
            "DW_DATABASE_URL",
            std::env::var("DW_TEST_DATABASE_URL").unwrap(),
        );
    for name in [
        "DW_INFERENCE_BROKER_DATABASE_URL",
        "DW_COGNITION_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL",
        "SystemRoot",
        "LLVM_PROFILE_FILE",
    ] {
        if let Some(v) = std::env::var_os(name) {
            child.env(name, v);
        }
    }
    let output = child
        .args([
            command,
            "--input",
            path.to_str().unwrap(),
            "--operation",
            &decision.to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fresh key-free CLI failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(
        output
            .stdout
            .split(|b| *b == b'\n')
            .rfind(|s| !s.is_empty())
            .unwrap(),
    )
    .unwrap()
}
fn infer_case(case: &mut native::NativeCase, provider: &mut EnvelopeProvider) -> Value {
    serde_json::to_value(infer(
        &mut case.broker,
        provider,
        &mut Basis,
        &mut case.store,
        &mut case.history,
        InferenceInput {
            scope: &case.scope,
            pack: &case.pack,
            event_id: Uuid::from_u128(case.scope.decision.as_u128() ^ 0x1d000000),
            occurred_at_ms: 1,
            source_mode: "blind".into(),
        },
    ))
    .unwrap()
}
fn calls(case: &mut native::NativeCase) -> Value {
    case.broker.client.query_one("SELECT coalesce(jsonb_agg(jsonb_build_object('call',call,'binding',binding,'state',state,'usage',usage) ORDER BY call),'[]') FROM execution.inference_calls WHERE d=$1", &[&case.scope.decision]).unwrap().get(0)
}
#[test]
fn production_decoder_to_pg_decision_and_fresh_cli_recovery() {
    let _guard = native::db();
    let mut case = native::decision(0x1d001);
    let mut provider = EnvelopeProvider::new(None);
    let receipt = infer_case(&mut case, &mut provider);
    assert_eq!(provider.sends, 1);
    assert_eq!(receipt["proposal_disposition"], "provider_incomplete");
    assert_eq!(receipt["publication"], "committed");
    assert_eq!(receipt["history"], "completed");
    assert_eq!(receipt["dispatch_granted"], false);
    assert!(receipt["proposal"].is_null());
    let before = calls(&mut case);
    assert_eq!(before.as_array().unwrap().len(), 1);
    assert_eq!(before[0]["state"], "usage_unknown");
    assert_eq!(
        before[0]["binding"],
        request::bind(&case.pack, None).unwrap().digest
    );
    assert_eq!(
        before[0]["usage"],
        serde_json::to_value(provider.diagnostic.report.as_ref().unwrap()).unwrap()
    );
    assert!(!before.to_string().contains("SENTINEL"));
    let path = std::env::temp_dir().join(format!("dw-terminal-{}.json", case.scope.decision));
    std::fs::write(&path, serde_json::to_vec(&case.request).unwrap()).unwrap();
    let counts = durable_counts(&case);
    assert_eq!(counts, (1, 1, 0));
    for command in ["m1-infer-inspect", "m1-infer-recover", "m1-infer-recover"] {
        let recovered = cli(command, &path, case.scope.decision);
        assert_eq!(recovered["provenance"], receipt["provenance"]);
        assert_eq!(recovered["call_ids"], receipt["call_ids"]);
        assert_eq!(recovered["proposal_disposition"], "provider_incomplete");
        assert_eq!(recovered["consumption"], "uncertain_charged");
        assert_eq!(calls(&mut case), before);
        assert_eq!(durable_counts(&case), counts);
    }
}
fn durable_counts(case: &native::NativeCase) -> (i64, i64, i64) {
    let row = native::admin().query_one("SELECT (SELECT count(*) FROM cognition.reasoning_decisions WHERE d=$1), (SELECT count(*) FROM trajectory.reasoning_history WHERE operation_id=$1), (SELECT count(*) FROM execution.fixture_dns_attempts WHERE decision=$1)",&[&case.scope.decision]).unwrap();
    (row.get(0), row.get(1), row.get(2))
}
#[test]
fn accounting_write_failure_preserves_unknown_without_another_send() {
    let _guard = native::db();
    let mut case = native::decision(0x1d002);
    let pid: i32 = case
        .broker
        .client
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    let mut provider = EnvelopeProvider::new(Some(pid));
    let receipt = infer_case(&mut case, &mut provider);
    assert_eq!(provider.sends, 1);
    assert_eq!(
        receipt["proposal_disposition"],
        "inference_accounting_unknown"
    );
    assert!(receipt["proposal"].is_null());
    assert_eq!(receipt["dispatch_granted"], false);
    case.broker.client = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let before = calls(&mut case);
    assert_eq!(before.as_array().unwrap().len(), 1);
    assert_eq!(before[0]["state"], "may_send_uncertain");
    assert!(before[0]["usage"].is_null());
    let path = std::env::temp_dir().join(format!("dw-terminal-{}.json", case.scope.decision));
    std::fs::write(&path, serde_json::to_vec(&case.request).unwrap()).unwrap();
    for command in ["m1-infer-inspect", "m1-infer-recover"] {
        let recovered = cli(command, &path, case.scope.decision);
        assert_eq!(recovered["consumption"], "uncertain_charged");
        assert_eq!(recovered["call_ids"], receipt["call_ids"]);
        assert_eq!(calls(&mut case), before);
    }
    assert_eq!(durable_counts(&case), (1, 1, 0));
}
fn write_report(client: &mut postgres::Client, call: Uuid, state: &str, report: &Value) -> bool {
    client
        .execute(
            "SELECT execution.inference_call_state($1,$2,$3)",
            &[&call, &state, report],
        )
        .is_ok()
}
#[test]
fn restricted_sql_validates_before_mutation_and_preserves_reports_and_legacy() {
    let _guard = native::db();
    // Successor refresh must retain the amended native definition.
    native::admin()
        .batch_execute(include_str!("../migrations/0021_inference_lifecycle.sql"))
        .unwrap();
    let mut case = native::decision(0x1d003);
    let reservation = case.broker.reserve(&case.scope, &case.pack).unwrap();
    let bound = request::bind(&case.pack, None).unwrap();
    case.broker
        .may_send(&case.scope, reservation.generation_call, &bound)
        .unwrap();
    let original = calls(&mut case);
    let safe = serde_json::to_value(decode(incomplete()).report.unwrap()).unwrap();
    for variable in [
        "DW_TEST_DATABASE_URL",
        "DW_COGNITION_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL",
        "DW_NATIVE_OTHER_BROKER_DATABASE_URL",
    ] {
        assert!(!write_report(
            &mut native::client(variable),
            reservation.generation_call,
            "usage_unknown",
            &safe
        ));
        assert_eq!(calls(&mut case), original);
    }
    let mut malformed = Vec::new();
    for (key, value) in [
        ("error", json!("SENTINEL")),
        ("input_tokens", json!(-1)),
        ("output_tokens", json!(1.5)),
        ("reasoning_tokens", json!(9223372036854775808u64)),
        ("response_bytes", json!(65537)),
        ("wall_time_ms", Value::Null),
        ("reported_model", json!("SENTINEL")),
        ("requested_model", Value::Null),
        ("provider_response", Value::Null),
    ] {
        let mut bad = safe.clone();
        bad[key] = value;
        malformed.push(bad);
    }
    for (key, value) in [
        ("status", json!("SENTINEL")),
        ("incomplete_reason", json!("SENTINEL")),
        ("usage_availability", json!("unavailable")),
        ("private", json!("SENTINEL")),
    ] {
        let mut bad = safe.clone();
        bad["provider_response"][key] = value;
        malformed.push(bad);
    }
    for key in ["input_tokens", "provider_response"] {
        let mut bad = safe.clone();
        bad.as_object_mut().unwrap().remove(key);
        malformed.push(bad);
    }
    let mut bad = safe.clone();
    bad["provider_response"]["status"] = json!("failed");
    malformed.push(bad);
    for bad in malformed {
        assert!(
            !write_report(
                &mut case.broker.client,
                reservation.generation_call,
                "usage_unknown",
                &bad
            ),
            "malformed safe projection was admitted"
        );
        assert_eq!(calls(&mut case), original);
    }
    assert!(write_report(
        &mut case.broker.client,
        reservation.generation_call,
        "usage_unknown",
        &safe
    ));
    let recorded = calls(&mut case);
    for value in [&safe, &Value::Null] {
        assert!(write_report(
            &mut case.broker.client,
            reservation.generation_call,
            "usage_unknown",
            value
        ));
        assert_eq!(calls(&mut case), recorded);
    }
    let mut conflicting = safe.clone();
    conflicting["input_tokens"] = json!(13);
    assert!(!write_report(
        &mut case.broker.client,
        reservation.generation_call,
        "usage_unknown",
        &conflicting
    ));
    assert_eq!(calls(&mut case), recorded);
    assert!(write_report(
        &mut case.broker.client,
        reservation.generation_call,
        "cancelled_uncertain",
        &Value::Null
    ));
    assert!(write_report(
        &mut case.broker.client,
        reservation.generation_call,
        "usage_unknown",
        &safe
    ));
    let cancelled = calls(&mut case);
    assert_eq!(cancelled[0]["state"], "cancelled_uncertain");
    assert_eq!(cancelled[0]["usage"], safe);
    // New code reads an ordinary predecessor-shaped completed call without rewriting it.
    let mut legacy = native::decision(0x1d004);
    let r = legacy.broker.reserve(&legacy.scope, &legacy.pack).unwrap();
    let b = request::bind(&legacy.pack, None).unwrap();
    legacy
        .broker
        .may_send(&legacy.scope, r.generation_call, &b)
        .unwrap();
    let usage = Usage {
        input_tokens: Some(1),
        output_tokens: None,
        reasoning_tokens: None,
        response_bytes: 7,
        wall_time_ms: 1,
        requested_model: "deepseek-flash",
        reported_model: None,
    };
    legacy
        .broker
        .completed(&legacy.scope, r.generation_call, &usage)
        .unwrap();
    let old = calls(&mut legacy);
    assert!(old[0]["usage"].get("provider_response").is_none());
    let provenance = legacy.broker.provenance(&legacy.scope).unwrap();
    assert_eq!(
        provenance[0]["reported"],
        serde_json::to_value(usage).unwrap()
    );
    assert_eq!(calls(&mut legacy), old);
}
