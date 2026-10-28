//! Cognition application ports, immutable decision and provider-free recovery.
use crate::m1_reasoning::proposal::{Admission, Proposal, ProposalBasis};
use crate::m1_reasoning::request::BoundCall;
use serde::Serialize;
use std::time::Instant;
use uuid::Uuid;

type Res<T> = Result<T, &'static str>;

#[derive(Clone)]
pub struct Scope {
    pub engagement: Uuid,
    pub campaign: Uuid,
    pub decision: Uuid,
    pub mission_revision: u64,
    pub permission: Uuid,
}

pub struct Reservation {
    pub generation: u64,
    pub generation_call: Uuid,
    pub repair_call: Uuid,
    pub shared_deadline: Instant,
}

#[derive(Clone, Debug, Serialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub response_bytes: usize,
    pub wall_time_ms: u64,
    pub requested_model: &'static str,
    pub reported_model: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Completed,
    Incomplete,
    Failed,
    InProgress,
    Unrecognized,
    Missing,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IncompleteReason {
    MaxOutputTokens,
    ContentFilter,
    Unknown,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageAvailability {
    Reported,
    Partial,
    Unavailable,
}
#[derive(Clone, Debug, Serialize)]
pub struct ResponseMetadata {
    pub status: ResponseStatus,
    pub incomplete_reason: Option<IncompleteReason>,
    pub usage_availability: UsageAvailability,
}
#[derive(Clone, Debug, Serialize)]
pub struct ProviderReport {
    #[serde(flatten)]
    pub usage: Usage,
    pub provider_response: ResponseMetadata,
}

pub struct Received {
    pub admission: Admission,
    pub usage: Usage,
    pub accounting_confirmed: bool,
}

pub enum ReleaseFailure {
    NeverReleased(&'static str),
    MayHaveSent(&'static str),
}

pub trait Provider {
    fn terminal_report(&self, _binding_digest: &str) -> Option<&ProviderReport> {
        None
    }
    fn prepare(&mut self, call: &BoundCall, deadline: Instant) -> Res<()>;
    // Concrete adapter keeps raw response/reasoning/error text transient and
    // invokes Cognition's admission inside that boundary before returning.
    fn call(
        &mut self,
        call: &BoundCall,
        deadline: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Res<()>,
    ) -> Res<Received>;
}

pub trait Broker {
    fn reserve(&mut self, scope: &Scope, pack: &[u8]) -> Res<Reservation>;
    fn may_send(&mut self, scope: &Scope, call_id: Uuid, bound: &BoundCall) -> Res<()>;
    // Implemented release must serialize actual provider start with local stop
    // after SQL ends, not return a reusable permission to an unrelated caller.
    fn release_into(
        &mut self,
        scope: &Scope,
        reservation: &Reservation,
        call_id: Uuid,
        bound: &BoundCall,
        provider: &mut dyn Provider,
        basis: &mut dyn ProposalBasis,
    ) -> Result<Received, ReleaseFailure>;
    fn never_released(&mut self, scope: &Scope, call_id: Uuid, category: &'static str) -> Res<()>;
    fn completed(&mut self, scope: &Scope, call_id: Uuid, usage: &Usage) -> Res<()>;
    fn uncertain(&mut self, scope: &Scope, call_id: Uuid, category: &'static str) -> Res<()>;
    fn uncertain_with_report(
        &mut self,
        scope: &Scope,
        call_id: Uuid,
        category: &'static str,
        _report: Option<&ProviderReport>,
    ) -> Res<()> {
        self.uncertain(scope, call_id, category)
    }
    fn close_unused_repair(&mut self, scope: &Scope, reservation: &Reservation) -> Res<()>;
    fn provenance(&mut self, scope: &Scope) -> Res<serde_json::Value>;
    fn finish(&mut self, scope: &Scope) -> Res<()>;
    fn inspect(&mut self, scope: &Scope) -> Res<&'static str>;
    fn inspect_pack(&mut self, _scope: &Scope) -> Res<Option<String>> {
        Ok(None)
    }
}

#[derive(Serialize)]
pub struct Decision {
    pub kind: &'static str,
    pub occurred_at_ms: u64,
    pub recorded_at_ms: u64,
    pub source_mode: String,
    pub provenance: serde_json::Value,
    pub event: Uuid,
    pub engagement: Uuid,
    pub campaign: Uuid,
    pub decision: Uuid,
    pub producer: &'static str,
    pub version: u32,
    pub mission_revision: u64,
    pub permission: Uuid,
    pub call_ids: Vec<Uuid>,
    pub bindings: Vec<String>,
    pub proposal: Option<Proposal>,
    pub disposition: &'static str,
    pub publication_obligation: &'static str,
}

pub trait DecisionStore {
    fn publish_atomic(&mut self, decision: &Decision) -> Res<()>;
    fn inspect(&mut self, scope: &Scope) -> Res<Option<Decision>>;
}

pub trait ReasoningHistory {
    fn accept_committed_source(&mut self, event: &Decision) -> Res<&'static str>;
    fn inspect(&mut self, event: &Decision) -> Res<&'static str>;
}

#[derive(Serialize)]
pub struct Receipt {
    pub kind: &'static str,
    pub call_ids: Vec<Uuid>,
    pub provenance: serde_json::Value,
    pub pack_state: &'static str,
    pub pack_utf8: Option<String>,
    pub proposal: Option<Proposal>,
    pub recovery_category: &'static str,
    pub decision: Uuid,
    pub event: Option<Uuid>,
    pub proposal_disposition: &'static str,
    pub publication: &'static str,
    pub history: &'static str,
    pub consumption: &'static str,
    pub dispatch_granted: bool,
}
pub struct InferenceInput<'a> {
    pub scope: &'a Scope,
    pub pack: &'a [u8],
    pub event_id: Uuid,
    pub occurred_at_ms: u64,
    pub source_mode: String,
}

pub fn infer(
    broker: &mut impl Broker,
    provider: &mut impl Provider,
    basis: &mut impl ProposalBasis,
    store: &mut impl DecisionStore,
    history: &mut impl ReasoningHistory,
    input: InferenceInput<'_>,
) -> Receipt {
    let InferenceInput {
        scope,
        pack,
        event_id,
        occurred_at_ms,
        source_mode,
    } = input;
    let (admission, ids, bindings) =
        match super::decision_attempt::decide(broker, provider, basis, scope, pack) {
            Ok(value) => value,
            Err(code) => (Admission::Rejected(code), Vec::new(), Vec::new()),
        };
    let successor = serde_json::from_slice::<serde_json::Value>(pack).is_ok_and(|v| {
        matches!(
            v["version"].as_str(),
            Some("m1_context_pack_autonomous_v2" | "m1_context_pack_autonomous_v3")
        )
    });
    let (proposal, disposition) = recorded_proposal(admission, successor, basis);
    let event = Decision {
        kind: "reasoning_decision",
        occurred_at_ms,
        recorded_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|v| v.as_millis() as u64)
            .unwrap_or(occurred_at_ms),
        source_mode,
        provenance: broker.provenance(scope).unwrap_or(serde_json::Value::Null),
        event: event_id,
        engagement: scope.engagement,
        campaign: scope.campaign,
        decision: scope.decision,
        producer: "cognition",
        version: if successor { 2 } else { 1 },
        mission_revision: scope.mission_revision,
        permission: scope.permission,
        call_ids: ids,
        bindings,
        proposal,
        disposition,
        publication_obligation: "trajectory.reasoning_history.v1",
    };
    let (publication, consumer) = publish(&event, scope, broker, store, history);
    let (pack_state, pack_utf8) = receipt_pack(broker.inspect_pack(scope));
    Receipt {
        kind: "m1_inference_receipt_v1",
        call_ids: event.call_ids,
        provenance: event.provenance,
        pack_state,
        pack_utf8,
        proposal: event.proposal,
        recovery_category: "inspect_original_identity",
        decision: scope.decision,
        event: Some(event_id),
        proposal_disposition: disposition,
        publication,
        history: consumer,
        consumption: broker.inspect(scope).unwrap_or("unknown"),
        dispatch_granted: false,
    }
}

pub fn inspect_recover(
    scope: &Scope,
    redeliver: bool,
    broker: &mut impl Broker,
    store: &mut impl DecisionStore,
    history: &mut impl ReasoningHistory,
) -> Res<Receipt> {
    let event = store.inspect(scope)?;
    let (call_ids, provenance, proposal) = event_fields(&event);
    let (event_id, disposition, publication, status) = recover_event(event, redeliver, history)?;
    let ((pack_state, pack_utf8), consumption) = inspect_broker(scope, redeliver, status, broker)?;
    Ok(Receipt {
        kind: "m1_inference_receipt_v1",
        call_ids,
        provenance,
        pack_state,
        pack_utf8,
        proposal,
        recovery_category: "no_provider_replay",
        decision: scope.decision,
        event: event_id,
        proposal_disposition: disposition,
        publication,
        history: status,
        consumption,
        dispatch_granted: false,
    })
}

fn event_fields(event: &Option<Decision>) -> (Vec<Uuid>, serde_json::Value, Option<Proposal>) {
    match event {
        Some(d) => (d.call_ids.clone(), d.provenance.clone(), d.proposal.clone()),
        None => (Vec::new(), serde_json::Value::Null, None),
    }
}

fn receipt_pack(res: Res<Option<String>>) -> (&'static str, Option<String>) {
    match res {
        Ok(Some(pack)) => ("available", Some(pack)),
        Ok(None) => ("not_bound", None),
        Err(_) => ("unavailable", None),
    }
}

fn inspect_broker(
    scope: &Scope,
    redeliver: bool,
    status: &'static str,
    broker: &mut impl Broker,
) -> Res<((&'static str, Option<String>), &'static str)> {
    if redeliver && status == "completed" {
        let _ = broker.finish(scope);
    }
    let pack = match broker.inspect_pack(scope) {
        Err(e) if e != "inference_inspect_unavailable" => return Err(e),
        res => receipt_pack(res),
    };
    Ok((pack, broker.inspect(scope)?))
}

fn admitted_proposal(admission: Admission) -> (Option<Proposal>, &'static str) {
    match admission {
        Admission::Accepted(p) => (Some(*p), "validated_proposal"),
        Admission::Repairable(_) => (None, "schema_repair_exhausted"),
        Admission::Rejected(code) => (None, code),
    }
}

fn recorded_proposal(
    admission: Admission,
    successor: bool,
    basis: &mut impl ProposalBasis,
) -> (Option<Proposal>, &'static str) {
    use super::proposal::{RecordedProposal, RecordingPremises};
    let Admission::Accepted(proposal) = admission else {
        return admitted_proposal(admission);
    };
    if !successor {
        return admitted_proposal(Admission::Accepted(proposal));
    }
    let recorded = match basis.recording_premises() {
        Ok(RecordingPremises::Fresh) => RecordedProposal::Current(proposal),
        Ok(RecordingPremises::ExpiredByTtl) => RecordedProposal::StalePremises(proposal),
        Err(code) => return (None, code),
    };
    match recorded {
        RecordedProposal::Current(p) => (Some(*p), "validated_proposal"),
        RecordedProposal::StalePremises(p) => (Some(*p), "recorded_stale_premises"),
    }
}
fn publish(
    event: &Decision,
    scope: &Scope,
    broker: &mut impl Broker,
    store: &mut impl DecisionStore,
    history: &mut impl ReasoningHistory,
) -> (&'static str, &'static str) {
    match store.publish_atomic(event) {
        Ok(()) => {
            let consumer = history.accept_committed_source(event).unwrap_or("unknown");
            if consumer == "completed" {
                let _ = broker.finish(scope);
            }
            ("committed", consumer)
        }
        Err(_) => ("unknown", "pending_unknown"),
    }
}
type RecoveryState = (Option<Uuid>, &'static str, &'static str, &'static str);
fn recover_event(
    event: Option<Decision>,
    redeliver: bool,
    history: &mut impl ReasoningHistory,
) -> Res<RecoveryState> {
    let Some(event) = event else {
        return Ok((
            None,
            "unknown",
            "absent_not_proof_of_no_send",
            "unavailable",
        ));
    };
    let status = if redeliver {
        history.accept_committed_source(&event)?
    } else {
        history.inspect(&event)?
    };
    Ok((Some(event.event), event.disposition, "committed", status))
}
