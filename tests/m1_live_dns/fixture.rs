//! Application-port controls. No provider, network, database or host qualification.
use duskweave::m1_reasoning::{inference::*, proposal::*, request::BoundCall};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use uuid::Uuid;
type Res<T> = Result<T, &'static str>;

pub fn scope() -> Scope {
    Scope {
        engagement: Uuid::from_u128(1),
        campaign: Uuid::from_u128(2),
        decision: Uuid::from_u128(3),
        permission: Uuid::from_u128(4),
        mission_revision: 1,
    }
}
pub fn pack() -> Vec<u8> {
    let s = scope();
    serde_json::to_vec(&json!({"version":"m1_context_pack_autonomous_v4",
        "run_profile":"autonomous-dns-genai-lab-v1","execution_mode":"live",
        "inference_profile":"fixture-genai-responses-v1","engagement_id":s.engagement,
        "live_binding":super::admission::binding("192.0.2.53"),
        "qualification_purpose":"LAB",
        "campaign_id":s.campaign,"decision_id":s.decision,"mission_revision":1,
        "permission_id":s.permission,"references":[{"id":"terrain_0"}],
        "manifests":[{"reference":"dns_udp"}]}))
    .unwrap()
}
pub struct Basis {
    pub released: Arc<AtomicBool>,
    pub recording_checked: bool,
    pub recording: Res<RecordingPremises>,
}
impl ProposalBasis for Basis {
    fn reference_eligible(&self, _: &str) -> bool {
        true
    }
    fn manifest_eligible(&self, _: &str) -> bool {
        true
    }
    fn current_authority_and_premises(&mut self) -> Res<()> {
        Ok(())
    }
    fn safe_explanation(&self, _: &str) -> bool {
        true
    }
    fn recording_premises(&mut self) -> Res<RecordingPremises> {
        assert!(
            self.released.load(Ordering::SeqCst),
            "recording follows release"
        );
        self.recording_checked = true;
        self.recording
    }
}
pub struct Sender(pub usize, pub Arc<AtomicBool>);
impl Provider for Sender {
    fn prepare(&mut self, _: &BoundCall, _: Instant) -> Res<()> {
        Ok(())
    }
    fn call(
        &mut self,
        _: &BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Res<()>,
    ) -> Res<Received> {
        release()?;
        assert!(
            !self.1.swap(true, Ordering::SeqCst),
            "only one generation release"
        );
        basis.before_provider_release()?;
        self.0 += 1;
        Ok(Received { admission: Admission::Accepted(Box::new(serde_json::from_value(json!({
            "version":"m1_inference_proposal_v1","hypothesis":"The issued routing evidence supports retention",
            "considered_alternative":"The route may change","references":["terrain_0"],
            "expected_observation":"One admitted A response","falsifying_observation":"No current binding",
            "uncertainty":"One vantage","rationale":"Retain the historical observation with its limits",
            "next_intent":{"category":"retain","references":["terrain_0"]}})).unwrap())),
            usage: Usage { input_tokens: Some(1), output_tokens: Some(1), reasoning_tokens: None,
                response_bytes: 1, wall_time_ms: 1, requested_model: "deepseek-flash", reported_model: None },
            accounting_confirmed: false })
    }
}
pub struct Ledger;
impl Broker for Ledger {
    fn reserve(&mut self, _: &Scope, _: &[u8]) -> Res<Reservation> {
        Ok(Reservation {
            generation: 1,
            generation_call: Uuid::from_u128(5),
            repair_call: Uuid::from_u128(6),
            shared_deadline: Instant::now() + Duration::from_secs(124),
        })
    }
    fn may_send(&mut self, _: &Scope, _: Uuid, _: &BoundCall) -> Res<()> {
        Ok(())
    }
    fn release_into(
        &mut self,
        _: &Scope,
        r: &Reservation,
        _: Uuid,
        b: &BoundCall,
        p: &mut dyn Provider,
        basis: &mut dyn ProposalBasis,
    ) -> Result<Received, ReleaseFailure> {
        p.call(b, r.shared_deadline, basis, &mut || Ok(()))
            .map_err(ReleaseFailure::MayHaveSent)
    }
    fn never_released(&mut self, _: &Scope, _: Uuid, _: &'static str) -> Res<()> {
        Ok(())
    }
    fn completed(&mut self, _: &Scope, _: Uuid, _: &Usage) -> Res<()> {
        Ok(())
    }
    fn uncertain(&mut self, _: &Scope, _: Uuid, _: &'static str) -> Res<()> {
        Ok(())
    }
    fn close_unused_repair(&mut self, _: &Scope, _: &Reservation) -> Res<()> {
        Ok(())
    }
    fn provenance(&mut self, _: &Scope) -> Res<Value> {
        Ok(json!([]))
    }
    fn finish(&mut self, _: &Scope) -> Res<()> {
        Ok(())
    }
    fn inspect(&mut self, _: &Scope) -> Res<&'static str> {
        Ok("completed_charged_unused_repair_reserved")
    }
}
#[derive(Default)]
pub struct Store(pub Option<Decision>);
impl DecisionStore for Store {
    fn publish_atomic(&mut self, d: &Decision) -> Res<()> {
        self.0 = Some(duskweave::m1_reasoning::postgres::decode(
            serde_json::to_value(d).unwrap(),
        )?);
        Ok(())
    }
    fn inspect(&mut self, _: &Scope) -> Res<Option<Decision>> {
        self.0
            .as_ref()
            .map(|d| duskweave::m1_reasoning::postgres::decode(serde_json::to_value(d).unwrap()))
            .transpose()
    }
}
pub struct History;
impl ReasoningHistory for History {
    fn accept_committed_source(&mut self, _: &Decision) -> Res<&'static str> {
        Ok("completed")
    }
    fn inspect(&mut self, _: &Decision) -> Res<&'static str> {
        Ok("completed")
    }
}
