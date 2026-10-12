//! Cognition-owned, read-only ContextPack assembly. No accepted decision or transport.
use crate::m1_context_capacity::ContextCapacity;
use crate::m1_context_input::ContextRequest;
use crate::m1_inference_permission::ContextDisclosure;
use crate::mission::{MissionRegistered, OperationId};
use crate::{Fail, Res};
use serde_json::{Value, json};

pub const QUESTION: &str = "Which already-authorized, mission-relevant HTTPS candidate is the best next subject for access-validation planning within the permitted contact envelope, and which routing or response premise still needs observation, a different candidate, or deferral?";

pub struct ContextBasis {
    pub mission: MissionRegistered,
    pub capacity: ContextCapacity,
    pub evaluated_at: i64,
}
pub trait ContextReader {
    fn read(&mut self, q: &ContextRequest, op: OperationId) -> Res<ContextBasis>;
}
pub struct PreparedContext {
    pub decision_id: OperationId,
    utf8: String,
}
impl PreparedContext {
    pub fn utf8(&self) -> &str {
        &self.utf8
    }
}

pub fn prepare(
    reader: &mut impl ContextReader,
    q: &ContextRequest,
    op: OperationId,
) -> Res<PreparedContext> {
    q.validate(op)?;
    let basis = reader.read(q, op)?;
    let disclosure = check_basis(&basis, q)?;
    fit_pack(&basis, &disclosure, q, op)
}

// Bounded optional-candidate omission: preserve required metadata even at zero.
fn fit_pack(
    basis: &ContextBasis,
    disclosure: &ContextDisclosure,
    q: &ContextRequest,
    op: OperationId,
) -> Res<PreparedContext> {
    let mut count = q.candidates.len().min(disclosure.candidates);
    loop {
        let raw = pack(basis, disclosure, q, op, count)?;
        let utf8 = serde_json::to_string(&raw).map_err(|_| Fail::Store("context_encode"))?;
        if utf8.len() <= disclosure.context_bytes {
            return Ok(PreparedContext {
                decision_id: op,
                utf8,
            });
        }
        if count == 0 {
            return Err(Fail::State("context_size_deferred"));
        }
        count -= 1;
    }
}

fn check_basis(b: &ContextBasis, q: &ContextRequest) -> Res<ContextDisclosure> {
    let m = &b.mission;
    let f = &m.fields;
    let p = f
        .m1_inference_permission()
        .ok_or(Fail::State("inference_consent_absent"))?;
    let d = p.context_disclosure();
    let wanted = crate::m1_inference_budget::ReservationRequest {
        version: "m1_inference_reservation_request_v1".into(),
        engagement_id: m.engagement_id,
        campaign_id: m.campaign_id,
        operator_ref: f.operator_ref,
        expected_mission_revision: 1,
        permission_id: d.permission_id,
        goal_ref: f.goal_ref,
        exercise_mode: f.exercise_mode,
    };
    if q.mission != wanted || (m.version, m.owner_revision) != (3, 1) {
        return Err(Fail::State("context_basis_mismatch"));
    }
    check_window(b, &d)?;
    check_candidates(q, m, d.operator_premises)?;
    Ok(d)
}

// Mission and consent intervals use the same owner-qualified database clock.
fn check_window(b: &ContextBasis, d: &ContextDisclosure) -> Res<()> {
    let f = &b.mission.fields;
    if !d.required_classes
        || b.evaluated_at < d.starts_at
        || b.evaluated_at >= d.ends_at
        || b.evaluated_at < f.starts_at()
        || b.evaluated_at >= f.ends_at()
    {
        return Err(Fail::State("context_consent_deferred"));
    }
    Ok(())
}

fn check_candidates(q: &ContextRequest, m: &MissionRegistered, allowed: bool) -> Res<()> {
    if !allowed && !q.candidates.is_empty() {
        return Err(Fail::State("context_disclosure_deferred"));
    }
    if q.candidates.iter().any(|c| {
        !m.fields.included_assets.contains(&c.asset_ref)
            || m.fields.excluded_assets.contains(&c.asset_ref)
    }) {
        return Err(Fail::Input("context_scope_refused"));
    }
    Ok(())
}

fn references(b: &ContextBasis, q: &ContextRequest, count: usize) -> Vec<Value> {
    let m = &b.mission;
    let mut refs = vec![
        json!({"id":"mission","owner":"mission","entity":m.campaign_id,
        "revision":m.owner_revision,"lineage":[m.event_id,m.operation_id],
        "perceived_at":m.occurred_at,"time_uncertainty":"unspecified_by_owner",
        "status":"owner_published","tier":null,"freshness":"as_of_snapshot","completeness":"scope_reference"}),
    ];
    refs.extend(q.candidates[..count].iter().enumerate().map(|(i, c)| {
        json!({
        "id":format!("asset_{i}"),"owner":"operator","entity":c.asset_ref,"revision":null,
        "lineage":[],"source":"caller_declared_premise","perceived_at":null,
        "time_uncertainty":"unknown","status":"premise","tier":null,
        "freshness":"unknown","completeness":"declared_only"})
    }));
    refs
}

fn pack(
    b: &ContextBasis,
    d: &ContextDisclosure,
    q: &ContextRequest,
    op: OperationId,
    count: usize,
) -> Res<Value> {
    let m = &b.mission;
    let remaining = b.capacity.remaining(d.totals)?;
    let required = json!({"calls":d.required[0],"request_bytes":d.required[1],
        "response_bytes":d.required[2],"output_token_capacity":d.required[3],
        "wall_time_ms":d.required[4]});
    let mut views = vec![
        json!({"owner":"mission","availability":"available","revision":1,
        "frontier":m.event_id,"evaluated_at":b.evaluated_at}),
    ];
    views.extend(
        [
            "cyber_terrain",
            "foothold_graph",
            "attack_path_view",
            "objective_state",
            "campaign_trajectory",
        ]
        .iter()
        .map(|owner| {
            json!({"owner":owner,"availability":"unavailable","revision":null,
            "frontier":null,"evaluated_at":b.evaluated_at,"reason":"not_loaded_by_this_profile"})
        }),
    );
    let candidates: Vec<_> = q.candidates[..count].iter().enumerate().map(|(i,c)|
        json!({"reference":format!("asset_{i}"),"priority":c.priority,"status":"operator_premise"})).collect();
    Ok(
        json!({"version":"m1_context_pack_v1","engagement_id":m.engagement_id,
        "campaign_id":m.campaign_id,"decision_id":op,"permission_id":d.permission_id,
        "mission_revision":1,"goal_ref":m.fields.goal_ref,"exercise_mode":m.fields.exercise_mode,
        "scope":{"owner":"mission","publication_ref":m.event_id},"question":QUESTION,
        "evaluated_at":b.evaluated_at,"owner_views":views,"references":references(b,q,count),
        "candidates":candidates,"omissions":{"supplied_candidates":q.candidates.len(),
            "shown_candidates":count,"omitted_candidates":q.candidates.len()-count,
            "selection":"input_order_prefix_with_count_and_byte_bounds","manifests":"catalog_unavailable"},
        "pending_unknown":["environmental_views_unavailable","operator_premises_unverified","send_admission_pending"],
        "manifests":[],"remaining_capacity":{"snapshot_epoch":b.capacity.epoch,
            "campaign":remaining,"required_for_full_decision":required,
            "exclusive_owner_busy":b.capacity.session_claimed || b.capacity.active_holds > 0,
            "reservation_granted":false,"shared_call_clock":"not_started"}}),
    )
}
