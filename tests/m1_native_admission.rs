use duskweave::m1_reasoning::{
    inference::*,
    postgres::decode,
    proposal::{Admission, ProposalBasis},
    provider_diagnostic::ProviderDiagnostic,
    request, response,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

const PACK: &[u8] = br#"{"references":[{"id":"ref-1"}]}"#;
struct Basis;
impl ProposalBasis for Basis {
    fn reference_eligible(&self, r: &str) -> bool {
        r == "ref-1"
    }
    fn manifest_eligible(&self, _: &str) -> bool {
        false
    }
    fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    fn safe_explanation(&self, text: &str) -> bool {
        !text.contains("SENTINEL")
    }
}
fn proposal() -> Value {
    json!({"version":"m1_inference_proposal_v1","hypothesis":"bounded hypothesis",
        "considered_alternative":"alternative","references":["ref-1"],
        "expected_observation":"expected","falsifying_observation":"falsifying",
        "uncertainty":"unknown","rationale":"safe explanation",
        "next_intent":{"category":"observe_more","references":["ref-1"],
        "manifest_reference":null,"view_request":null}})
}
fn envelope() -> Vec<u8> {
    serde_json::to_vec(&json!({"status":"completed","model":"deepseek-flash",
        "output":[{"type":"reasoning","private":"SENTINEL"},
        {"type":"message","role":"assistant","status":"completed",
        "content":[{"type":"output_text","text":proposal().to_string()}]}],
        "usage":{"input_tokens":12,"output_tokens":23}}))
    .unwrap()
}
fn http(headers: &str) -> Vec<u8> {
    let mut raw = headers.as_bytes().to_vec();
    raw.extend(envelope());
    raw
}
#[test]
fn request_reference_and_repair_contract() {
    let first = request::bind(PACK, None).unwrap();
    let second = request::bind(PACK, Some(&["json_structure"])).unwrap();
    assert_eq!(first.pack, second.pack);
    assert_ne!(first.digest, second.digest);
    let body: Value = serde_json::from_slice(&first.body).unwrap();
    assert_eq!(body["model"], "deepseek-flash");
    assert_eq!(body["input"].as_str().unwrap().as_bytes(), PACK);
    assert!(
        String::from_utf8(first.configuration)
            .unwrap()
            .contains("ref-1")
    );
    assert!(matches!(
        request::bind(PACK, Some(&["reference_ineligible"])),
        Err("repair_code_ineligible")
    ));
    assert!(matches!(
        request::bind(PACK, Some(&["json_structure"; 9])),
        Err("repair_code_limit")
    ));
    assert!(request::reference_ids(br#"{"references":[{"id":"same"},{"id":"same"}]}"#).is_err());
}
#[test]
fn http_interim_identity_and_private_reasoning() {
    let raw =
        http("HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nContent-Encoding: identity\r\n\r\n");
    let mut d = ProviderDiagnostic::default();
    let received = response::admit_http(&raw, Duration::ZERO, &mut Basis, &mut d).unwrap();
    assert!(matches!(received.admission, Admission::Accepted(_)));
    assert_eq!(d.interim_responses, 1);
    assert_eq!(d.final_http_status, Some(200));
    assert_eq!(received.usage.output_tokens, Some(23));
    assert!(
        !serde_json::to_string(&received.usage)
            .unwrap()
            .contains("SENTINEL")
    );
}
#[test]
fn malformed_upgrade_encoding_and_response_limit_are_refused() {
    for header in [
        "HTTP/1.0 200 OK\r\n\r\n",
        "HTTP/1.1 101 Switching\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\n\r\n",
        "HTTP/1.1 200 OK\r\nBad Header: x\r\n\r\n",
        "HTTP/1.1 200 OK\r\nno-colon\r\n\r\n",
    ] {
        assert!(
            response::admit_http(
                &http(header),
                Duration::ZERO,
                &mut Basis,
                &mut ProviderDiagnostic::default()
            )
            .is_err()
        );
    }
    assert!(matches!(
        response::admit_http(
            &vec![b'x'; 65537],
            Duration::ZERO,
            &mut Basis,
            &mut ProviderDiagnostic::default()
        ),
        Err("provider_response_limit")
    ));
}
#[test]
fn duplicate_messages_and_unexpected_tools_are_refused() {
    let mut v: Value = serde_json::from_slice(&envelope()).unwrap();
    let message = v["output"][1].clone();
    v["output"].as_array_mut().unwrap().push(message);
    assert!(
        response::admit_envelope(
            &serde_json::to_vec(&v).unwrap(),
            100,
            Duration::ZERO,
            &mut Basis
        )
        .is_err()
    );
    v["output"] = json!([{"type":"function_call"}]);
    assert!(
        response::admit_envelope(
            &serde_json::to_vec(&v).unwrap(),
            100,
            Duration::ZERO,
            &mut Basis
        )
        .is_err()
    );
}
fn stored() -> Value {
    json!({"kind":"reasoning_decision","occurred_at_ms":1,"recorded_at_ms":2,
        "source_mode":"blind","provenance":null,"event":Uuid::from_u128(1),
        "engagement":Uuid::from_u128(2),"campaign":Uuid::from_u128(3),"decision":Uuid::from_u128(4),
        "producer":"cognition","version":1,"mission_revision":1,"permission":Uuid::from_u128(5),
        "call_ids":[],"bindings":[],"proposal":null,"disposition":"reference_ineligible",
        "publication_obligation":"trajectory.reasoning_history.v1"})
}
#[test]
fn stored_decision_reconstruction_and_closed_metadata() {
    let d = decode(stored()).unwrap();
    assert_eq!(d.disposition, "reference_ineligible");
    assert_eq!(d.event, Uuid::from_u128(1));
    for (field, invalid) in [
        ("kind", json!("forged")),
        ("producer", json!("broker")),
        ("source_mode", json!("unknown")),
        ("event", json!("bad-uuid")),
        ("disposition", json!("made_up")),
        ("version", json!(2)),
    ] {
        let mut v = stored();
        v[field] = invalid;
        assert!(matches!(decode(v), Err("inference_decision_integrity")));
    }
}
#[test]
fn stored_decision_unknown_metadata_is_integrity_failure() {
    let mut v = stored();
    v["extra_metadata"] = json!(true);
    assert!(matches!(decode(v), Err("inference_decision_integrity")));
}
struct FakeProvider {
    calls: usize,
    repair: bool,
}
impl Provider for FakeProvider {
    fn prepare(&mut self, _: &request::BoundCall, _: Instant) -> Result<(), &'static str> {
        Ok(())
    }
    fn call(
        &mut self,
        _: &request::BoundCall,
        _: Instant,
        _: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        release()?;
        self.calls += 1;
        Ok(Received {
            admission: if self.repair && self.calls == 1 {
                Admission::Repairable(vec!["json_structure"])
            } else {
                Admission::Rejected("reference_ineligible")
            },
            accounting_confirmed: false,
            usage: Usage {
                input_tokens: None,
                output_tokens: None,
                reasoning_tokens: None,
                response_bytes: 0,
                wall_time_ms: 0,
                requested_model: "deepseek-flash",
                reported_model: None,
            },
        })
    }
}
struct FakeBroker {
    marker: Option<&'static str>,
    uncertain_usage: bool,
    markers: usize,
}
impl Broker for FakeBroker {
    fn reserve(&mut self, _: &Scope, _: &[u8]) -> Result<Reservation, &'static str> {
        Ok(Reservation {
            generation: 1,
            generation_call: Uuid::from_u128(10),
            repair_call: Uuid::from_u128(11),
            shared_deadline: Instant::now() + Duration::from_secs(1),
        })
    }
    fn may_send(&mut self, _: &Scope, _: Uuid, _: &request::BoundCall) -> Result<(), &'static str> {
        self.markers += 1;
        self.marker.map_or(Ok(()), Err)
    }
    fn release_into(
        &mut self,
        _: &Scope,
        r: &Reservation,
        _: Uuid,
        b: &request::BoundCall,
        p: &mut dyn Provider,
        basis: &mut dyn ProposalBasis,
    ) -> Result<Received, ReleaseFailure> {
        p.call(b, r.shared_deadline, basis, &mut || Ok(()))
            .map_err(ReleaseFailure::MayHaveSent)
    }
    fn never_released(&mut self, _: &Scope, _: Uuid, _: &'static str) -> Result<(), &'static str> {
        Ok(())
    }
    fn completed(&mut self, _: &Scope, _: Uuid, _: &Usage) -> Result<(), &'static str> {
        if self.uncertain_usage {
            Err("usage_commit_unknown")
        } else {
            Ok(())
        }
    }
    fn uncertain(&mut self, _: &Scope, _: Uuid, _: &'static str) -> Result<(), &'static str> {
        Ok(())
    }
    fn close_unused_repair(&mut self, _: &Scope, _: &Reservation) -> Result<(), &'static str> {
        Ok(())
    }
    fn provenance(&mut self, _: &Scope) -> Result<Value, &'static str> {
        Ok(Value::Null)
    }
    fn finish(&mut self, _: &Scope) -> Result<(), &'static str> {
        Ok(())
    }
    fn inspect(&mut self, _: &Scope) -> Result<&'static str, &'static str> {
        Ok("uncertain_charged")
    }
}
#[derive(Default)]
struct FakeStore {
    saved: Option<Decision>,
}
impl DecisionStore for FakeStore {
    fn publish_atomic(&mut self, d: &Decision) -> Result<(), &'static str> {
        self.saved = Some(decode(serde_json::to_value(d).unwrap()).unwrap());
        Ok(())
    }
    fn inspect(&mut self, _: &Scope) -> Result<Option<Decision>, &'static str> {
        Ok(None)
    }
}
struct FakeHistory;
impl ReasoningHistory for FakeHistory {
    fn accept_committed_source(&mut self, _: &Decision) -> Result<&'static str, &'static str> {
        Ok("completed")
    }
    fn inspect(&mut self, _: &Decision) -> Result<&'static str, &'static str> {
        Ok("completed")
    }
}
fn exercise(
    marker: Option<&'static str>,
    uncertain_usage: bool,
    repair: bool,
) -> (FakeProvider, Decision) {
    let scope = Scope {
        engagement: Uuid::from_u128(2),
        campaign: Uuid::from_u128(3),
        decision: Uuid::from_u128(4),
        mission_revision: 1,
        permission: Uuid::from_u128(5),
    };
    let mut broker = FakeBroker {
        marker,
        uncertain_usage,
        markers: 0,
    };
    let mut provider = FakeProvider { calls: 0, repair };
    let mut store = FakeStore::default();
    let receipt = duskweave::m1_reasoning::inference::infer(
        &mut broker,
        &mut provider,
        &mut Basis,
        &mut store,
        &mut FakeHistory,
        InferenceInput {
            scope: &scope,
            pack: PACK,
            event_id: Uuid::from_u128(1),
            occurred_at_ms: 1,
            source_mode: "blind".into(),
        },
    );
    assert!(!receipt.dispatch_granted);
    (provider, store.saved.unwrap())
}
#[test]
fn generation_repair_preserves_exact_call_and_binding_membership() {
    let (provider, decision) = exercise(None, false, true);
    assert_eq!(provider.calls, 2);
    assert_eq!(decision.call_ids.len(), 2);
    assert_eq!(decision.bindings.len(), 2);
    assert_ne!(decision.bindings[0], decision.bindings[1]);
    assert_eq!(decision.disposition, "reference_ineligible");
}
#[test]
fn marker_refusal_and_commit_uncertainty_never_send() {
    for (code, bindings) in [
        ("inference_marker_refused", 0),
        ("inference_marker_commit_unknown", 1),
    ] {
        let (provider, decision) = exercise(Some(code), false, true);
        assert_eq!(provider.calls, 0);
        assert_eq!(decision.bindings.len(), bindings);
        assert_eq!(decision.disposition, code);
    }
}
#[test]
fn uncertain_accounting_blocks_repair_without_losing_recorded_binding() {
    let (provider, decision) = exercise(None, true, true);
    assert_eq!(provider.calls, 1);
    assert_eq!(decision.bindings.len(), 1);
    assert_eq!(decision.disposition, "accounting_unknown");
}

#[test]
fn diagnostic_and_bounded_json_edge_cases() {
    let d = ProviderDiagnostic::new("digest-1".into());
    assert_eq!(d.binding_digest, "digest-1");

    use duskweave::m1_reasoning::bounded_json::{self, JsonFailureKind};
    assert_eq!(bounded_json::parse(b"", 100, 100), Err("json_size"));
    assert_eq!(
        bounded_json::parse(b"{} extra", 100, 100),
        Err("json_trailing")
    );
    assert_eq!(
        bounded_json::parse(b"{bad", 100, 100),
        Err("json_structure")
    );

    let dup = bounded_json::parse_detailed(br#"{"a":1,"a":2}"#, 100, 100).unwrap_err();
    assert_eq!(dup.kind, JsonFailureKind::DuplicateKey);

    let long_key = format!(r#"{{"{}":1}}"#, "k".repeat(65));
    let k_err = bounded_json::parse_detailed(long_key.as_bytes(), 200, 100).unwrap_err();
    assert_eq!(k_err.kind, JsonFailureKind::KeyBytes);

    let t_err = bounded_json::parse_detailed(br#"{"a":"toolong"}"#, 100, 3).unwrap_err();
    assert_eq!(t_err.kind, JsonFailureKind::TextBytes);

    let big_arr = format!("[{}]", (0..66).map(|_| "1").collect::<Vec<_>>().join(","));
    let c_err = bounded_json::parse_detailed(big_arr.as_bytes(), 1000, 100).unwrap_err();
    assert_eq!(c_err.kind, JsonFailureKind::Collection);

    let deep = b"[[[[[[[[[[1]]]]]]]]]]";
    let d_err = bounded_json::parse_detailed(deep, 100, 100).unwrap_err();
    assert_eq!(d_err.kind, JsonFailureKind::Depth);

    let valid = br#"{"i":-42,"u":42,"f":3.14,"b":true,"n":null}"#;
    assert!(bounded_json::parse_detailed(valid, 100, 100).is_ok());
    assert!(bounded_json::parse_envelope_detailed(valid).is_ok());
}
