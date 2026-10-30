//! Closed decoding of persisted reasoning decisions.
use super::*;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredDecision {
    kind: String,
    occurred_at_ms: u64,
    recorded_at_ms: u64,
    source_mode: String,
    provenance: Value,
    event: Uuid,
    engagement: Uuid,
    campaign: Uuid,
    decision: Uuid,
    producer: String,
    version: u32,
    mission_revision: u64,
    permission: Uuid,
    call_ids: Vec<Uuid>,
    bindings: Vec<String>,
    proposal: Option<crate::m1_reasoning::proposal::Proposal>,
    disposition: String,
    publication_obligation: String,
}
const DISPOSITIONS: &[&str] = &[
    "recorded_stale_premises",
    "validated_proposal",
    "schema_repair_exhausted",
    "unsafe_explanation",
    "unsupported_premise",
    "reference_ineligible",
    "manifest_ineligible",
    "view_selector_ineligible",
    "current_authority_unavailable",
    "repair_code_ineligible",
    "repair_code_limit",
    "pack_size",
    "pack_utf8",
    "repair_encode",
    "request_encode",
    "request_size",
    "inference_marker_commit_unknown",
    "inference_marker_refused",
    "inference_marker_unknown",
    "inference_sql_ambiguous",
    "inference_sql_privilege",
    "inference_sql_function_missing",
    "stopped_before_release",
    "inference_stop_unknown",
    "inference_authority_unavailable",
    "inference_reservation_unknown",
    "inference_reservation_missing",
    "inference_deadline",
    "inference_guard_missing",
    "inference_platform_unqualified",
    "inference_guard_busy",
    "inference_fixture_endpoint_refused",
    "live_key_missing",
    "live_key_invalid",
    "inference_curl_missing",
    "inference_pack_encode",
    "inference_config_encode",
    "inference_body_encode",
    "inference_usage_encode",
    "inference_accounting_unknown",
    "inference_database_config_missing",
    "inference_database_config_invalid",
    "inference_database_unavailable",
    "provider_incomplete",
    "provider_protocol",
    "provider_unsafe_response",
    "provider_response_limit",
    "provider_transport_unknown",
    "provider_cancelled_uncertain",
    "accounting_unknown",
    "inference_release_unknown",
    "inference_transport_unavailable",
    "inference_inspect_unavailable",
];
pub fn decode(v: Value) -> Res<Decision> {
    let d: StoredDecision =
        serde_json::from_value(v).map_err(|_| "inference_decision_integrity")?;
    let metadata = [
        d.kind == "reasoning_decision",
        d.producer == "cognition",
        matches!(d.version, 1 | 2),
        d.version == 1 || d.provenance.is_array(),
        d.disposition != "recorded_stale_premises" || (d.version == 2 && d.proposal.is_some()),
        d.publication_obligation == "trajectory.reasoning_history.v1",
        ["blind", "defender_informed"].contains(&d.source_mode.as_str()),
    ];
    if !metadata.into_iter().all(|valid| valid) {
        return Err("inference_decision_integrity");
    }
    let disposition = DISPOSITIONS
        .iter()
        .copied()
        .find(|code| *code == d.disposition)
        .ok_or("inference_decision_integrity")?;
    Ok(Decision {
        kind: "reasoning_decision",
        occurred_at_ms: d.occurred_at_ms,
        recorded_at_ms: d.recorded_at_ms,
        source_mode: d.source_mode,
        provenance: d.provenance,
        event: d.event,
        engagement: d.engagement,
        campaign: d.campaign,
        decision: d.decision,
        producer: "cognition",
        version: d.version,
        mission_revision: d.mission_revision,
        permission: d.permission,
        call_ids: d.call_ids,
        bindings: d.bindings,
        proposal: d.proposal,
        disposition,
        publication_obligation: "trajectory.reasoning_history.v1",
    })
}
