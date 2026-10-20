//! Explicit local inference double. Its second proposal depends on the supplied qualified feedback.
use super::inference::{Provider, Received, Usage};
use super::proposal::{self, ProposalBasis};
use super::request::BoundCall;
use serde_json::{Value, json};
use std::time::Instant;

#[derive(Default)]
pub struct DeterministicProvider;
impl Provider for DeterministicProvider {
    fn prepare(&mut self, call: &BoundCall, deadline: Instant) -> Result<(), &'static str> {
        let p: Value = serde_json::from_slice(&call.pack).map_err(|_| "request_encode")?;
        let valid = [
            p["run_profile"] == "autonomous-dns-fixture-v1",
            p["execution_mode"] == "controlled_fixture",
            Instant::now() < deadline,
        ];
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or("inference_fixture_endpoint_refused")
    }
    fn call(
        &mut self,
        call: &BoundCall,
        deadline: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        self.prepare(call, deadline)?;
        release()?;
        let pack: Value = serde_json::from_slice(&call.pack).map_err(|_| "request_encode")?;
        let raw = proposal_for(&pack)?;
        let bytes = serde_json::to_vec(&raw).map_err(|_| "request_encode")?;
        Ok(Received {
            admission: proposal::admit(&bytes, basis),
            accounting_confirmed: false,
            usage: Usage {
                input_tokens: None,
                output_tokens: None,
                reasoning_tokens: None,
                response_bytes: bytes.len(),
                wall_time_ms: 0,
                requested_model: "deterministic-autonomous-v1",
                reported_model: Some("deterministic-autonomous-v1".into()),
            },
        })
    }
}
fn proposal_for(pack: &Value) -> Result<Value, &'static str> {
    let (references, intent, summary) = if pack["predecessor"]["availability"] == "committed" {
        feedback(pack)?
    } else {
        let seed = pack["candidates"][0]["reference"]
            .as_str()
            .ok_or("unsupported_premise")?;
        (
            json!([seed]),
            json!({"category":"observe_more","references":[seed],"manifest_reference":"dns_udp","view_request":null}),
            "The admitted seed needs a bounded fixture routing observation.",
        )
    };
    Ok(
        json!({"version":"m1_inference_proposal_v1","hypothesis":summary,
        "considered_alternative":"The routing premise may be unavailable.","references":references,
        "expected_observation":"An owner-qualified routing outcome with explicit fixture provenance.",
        "falsifying_observation":"Missing, expired or conflicted outcome.","uncertainty":"Fixture plumbing only; no live target fact or cognitive qualification.",
        "rationale":summary,"next_intent":intent}),
    )
}
fn feedback(pack: &Value) -> Result<(Value, Value, &'static str), &'static str> {
    let claim = pack["references"]
        .as_array()
        .ok_or("request_encode")?
        .iter()
        .find(|r| {
            r["owner"] == "cyber_terrain"
                && r["source"] == "fixture_dns_acquisition"
                && r["status"] == "observed"
        });
    let id = claim
        .and_then(|r| r["id"].as_str())
        .ok_or("reference_ineligible")?;
    Ok((
        json!(["previous_decision", id]),
        json!({"category":"retain","references":[id],"manifest_reference":null}),
        "The newly qualified fixture outcome supports retaining the routing hypothesis; live applicability remains unknown.",
    ))
}
