//! Terrain-owned HTTPS observation, evaluation, current view and event types.
pub use crate::m1_https::{
    HttpsHeadResult, HttpsOutcomeKind, HttpsSourceKind, RedirectDisposition,
};
use crate::m1_policy::{NamePurpose, PolicyReason};
pub use crate::m1_terrain_input::{TerrainClaimStatus, TerrainIds, TrustTier};
use crate::mission::{AssetRef, CampaignId, EngagementId, EventId, MissionRegistered, OperationId};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CLAIM_KIND_HTTPS_OBSERVATION: &str = "https_endpoint_head_observation";
pub const EVENT_KIND_TERRAIN_CHANGED: &str = "terrain_changed";
pub const OBLIGATION_TERRAIN_HISTORY: &str = "trajectory.terrain_history.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpsEvaluationOutcome {
    QualifiedHttpsObservation,
    ScopeOrPolicyWithheld,
    ExecutionRefused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainHttpsObservation {
    pub ids: TerrainIds,
    pub result: HttpsHeadResult,
    pub trusted_tier: TrustTier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainHttpsEvaluation {
    pub evaluation_id: Uuid,
    pub observation_id: Uuid,
    pub material_id: Uuid,
    pub claim_id: Uuid,
    pub outcome: HttpsEvaluationOutcome,
    pub usable_endpoint_premise: bool,
    pub endpoint_reachable: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainHttpsClaimView {
    pub claim_id: Uuid,
    pub claim_revision: u32,
    pub evaluation_id: Uuid,
    pub observation_id: Uuid,
    pub material_id: Uuid,
    pub asset_ref: AssetRef,
    pub hostname: String,
    pub ipv4: String,
    pub port: u16,
    pub approved_path: String,
    pub method: String,
    pub parent_claim_id: Uuid,
    pub parent_claim_revision: u32,
    pub claim_kind: String,
    pub source_kind: HttpsSourceKind,
    pub trust_tier: TrustTier,
    pub claim_status: TerrainClaimStatus,
    pub outcome: HttpsOutcomeKind,
    pub status_code: Option<u16>,
    pub redirect: RedirectDisposition,
    pub endpoint_reachable: bool,
    pub whole_host_proven: bool,
    pub objective_completed: bool,
    pub perceived_at: i64,
    pub evaluated_at: i64,
    pub uncertainty_seconds: u32,
    pub temporal_frontier: i64,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainHttpsChangedEvent {
    pub kind: String,
    pub publication_obligation: String,
    pub event_id: EventId,
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub operation_id: OperationId,
    pub evaluation: TerrainHttpsEvaluation,
    pub claim: TerrainHttpsClaimView,
}

fn observation_outcome(
    mission: &MissionRegistered,
    r: &HttpsHeadResult,
) -> Res<HttpsEvaluationOutcome> {
    let same_scope =
        (r.engagement_id, r.campaign_id) == (mission.engagement_id, mission.campaign_id);
    if !same_scope {
        return Err(Fail::Input("https_observation_campaign_mismatch"));
    }
    let scope_ok = mission.fields.included_assets.contains(&r.asset_ref)
        && !mission.fields.excluded_assets.contains(&r.asset_ref);
    let perm = mission
        .fields
        .m1_permission()
        .ok_or(Fail::State("https_observation_permission_absent"))?;
    let name_ok = perm.query_reason(
        &NamePurpose::Contact {
            name: r.hostname.clone(),
            asset_ref: r.asset_ref,
        },
        r.perceived_at,
    ) == PolicyReason::MatchesNamePolicy;
    Ok(if r.outcome == HttpsOutcomeKind::ExecutionRefused {
        HttpsEvaluationOutcome::ExecutionRefused
    } else if scope_ok && name_ok {
        HttpsEvaluationOutcome::QualifiedHttpsObservation
    } else {
        HttpsEvaluationOutcome::ScopeOrPolicyWithheld
    })
}

pub fn evaluate_observation(
    mission: &MissionRegistered,
    obs: &TerrainHttpsObservation,
    claim_revision: u32,
    evaluated_at: i64,
) -> Res<(
    TerrainHttpsEvaluation,
    TerrainHttpsClaimView,
    TerrainHttpsChangedEvent,
)> {
    obs.ids.validate()?;
    obs.result.validate()?;
    if claim_revision == 0 || evaluated_at < obs.result.perceived_at {
        return Err(Fail::Input("invalid_https_terrain_observation"));
    }
    let r = &obs.result;
    let outcome = observation_outcome(mission, r)?;
    let accepted = outcome == HttpsEvaluationOutcome::QualifiedHttpsObservation;
    let endpoint_reachable = r.outcome == HttpsOutcomeKind::HeadResponse;
    let eval = TerrainHttpsEvaluation {
        evaluation_id: obs.ids.evaluation_id,
        observation_id: obs.ids.observation_id,
        material_id: obs.ids.material_id,
        claim_id: obs.ids.claim_id,
        outcome,
        usable_endpoint_premise: accepted,
        endpoint_reachable: accepted.then_some(endpoint_reachable),
    };
    let claim = TerrainHttpsClaimView {
        claim_id: obs.ids.claim_id,
        claim_revision,
        evaluation_id: obs.ids.evaluation_id,
        observation_id: obs.ids.observation_id,
        material_id: obs.ids.material_id,
        asset_ref: r.asset_ref,
        hostname: r.hostname.clone(),
        ipv4: r.ipv4.clone(),
        port: r.port,
        approved_path: r.approved_path.clone(),
        method: r.method.clone(),
        parent_claim_id: r.parent_claim_id,
        parent_claim_revision: r.parent_claim_revision,
        claim_kind: CLAIM_KIND_HTTPS_OBSERVATION.into(),
        source_kind: r.source_kind,
        trust_tier: obs.trusted_tier,
        claim_status: if accepted {
            TerrainClaimStatus::Observed
        } else {
            TerrainClaimStatus::Withheld
        },
        outcome: r.outcome,
        status_code: r.status_code,
        redirect: r.redirect,
        endpoint_reachable,
        whole_host_proven: false,
        objective_completed: false,
        perceived_at: r.perceived_at,
        evaluated_at,
        uncertainty_seconds: r.uncertainty_seconds,
        temporal_frontier: r.perceived_at,
        limitations: r.limitations.clone(),
    };
    let event = TerrainHttpsChangedEvent {
        kind: EVENT_KIND_TERRAIN_CHANGED.into(),
        publication_obligation: OBLIGATION_TERRAIN_HISTORY.into(),
        event_id: obs.ids.event_id,
        engagement_id: mission.engagement_id,
        campaign_id: mission.campaign_id,
        operation_id: obs.ids.operation_id,
        evaluation: eval.clone(),
        claim: claim.clone(),
    };
    Ok((eval, claim, event))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::{Mission, RegistrationFields, RegistrationInput};

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn mission(asset: u128) -> MissionRegistered {
        let fields_v = serde_json::json!({
            "operator_ref": id(3),
            "authority_ref": id(4), "authority_revision": 1, "goal_ref": id(5),
            "exercise_mode": "blind",
            "included_assets": [id(asset)], "excluded_assets": [],
            "starts_at": 1_700_000_000i64, "ends_at": 1_700_001_000i64,
            "m1_permission": {
                "policy_version": 1, "ct_base_domain": "example.invalid",
                "provider_disclosure": "crt_sh",
                "vantage_ref": id(7), "resolver_ipv4": "192.0.2.53",
                "discovery_rules": [{"exact":"example.invalid"}],
                "contact_rules": [{"asset_ref": id(asset), "rule": {"exact":"target.example.invalid"}, "priority": 1}],
                "excluded_names": [], "approved_path": "/",
                "starts_at": 1_700_000_000, "ends_at": 1_700_001_000,
                "campaign_limits": {"episodes": 4, "provider_calls": 10, "dns_questions": 4, "dns_followups": 4, "tcp_connections": 4, "head_requests": 4},
                "concurrency": 1
            }
        });
        let fields: RegistrationFields = serde_json::from_value(fields_v).unwrap();
        let input = RegistrationInput {
            engagement_id: EngagementId(id(1)),
            campaign_id: CampaignId(id(2)),
            fields,
        };
        Mission::register(&input, OperationId(id(9)), EventId(id(8)), 1_700_000_000)
            .unwrap()
            .1
    }

    #[test]
    fn clear_connection_failure_and_head_response_preserve_limitations() {
        let m = mission(10);
        let obs = TerrainHttpsObservation {
            ids: TerrainIds {
                event_id: EventId(id(100)),
                operation_id: OperationId(id(101)),
                observation_id: id(102),
                material_id: id(103),
                evaluation_id: id(104),
                claim_id: id(105),
            },
            result: HttpsHeadResult {
                version: crate::m1_https::VERSION_HEAD_RESULT.into(),
                engagement_id: m.engagement_id,
                campaign_id: m.campaign_id,
                run_id: id(106),
                decision_id: OperationId(id(107)),
                attempt_id: id(108),
                result_id: id(109),
                asset_ref: AssetRef(id(10)),
                endpoint_ref: "endpoint_0".into(),
                hostname: "target.example.invalid".into(),
                ipv4: "198.51.100.10".into(),
                port: 443,
                approved_path: "/".into(),
                method: "HEAD".into(),
                parent_claim_id: id(110),
                parent_claim_revision: 1,
                perceived_at: 1_700_000_050,
                uncertainty_seconds: 1,
                outcome: HttpsOutcomeKind::ConnectionFailed,
                status_code: None,
                redirect: RedirectDisposition::None,
                source_kind: HttpsSourceKind::FixtureHttpsAcquisition,
                limitations: vec!["single_endpoint_connection_failed_not_whole_host".into()],
            },
            trusted_tier: TrustTier::Tier1,
        };
        let (eval, claim, event) = evaluate_observation(&m, &obs, 1, 1_700_000_051).unwrap();
        assert_eq!(
            eval.outcome,
            HttpsEvaluationOutcome::QualifiedHttpsObservation
        );
        assert_eq!(eval.endpoint_reachable, Some(false));
        assert!(!claim.whole_host_proven);
        assert!(!claim.objective_completed);
        assert_eq!(event.kind, EVENT_KIND_TERRAIN_CHANGED);
    }
}
