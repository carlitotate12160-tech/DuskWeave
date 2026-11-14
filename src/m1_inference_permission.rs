//! Mission-owned initial inference consent. Stored only with a new registration;
//! configured disclosure is historical evidence, never current send permission.
use crate::mission::*;
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SafeFieldClass {
    MissionQuestionConstraints,
    ScopedOpaqueReferences,
    OwnerQualifiedSummaries,
    AdmittedOperatorPremises,
    ManifestAvailability,
    OmissionsUnknown,
    Capacity,
    AdmittedRepairFieldsCodes,
    AdmittedCampaignDecisionFeedback,
}

mod limits;
pub(crate) use limits::ContextDisclosure;
use limits::{CampaignInferenceLimits, OperationInferenceLimits, present};

/// Closed permission for bounded single-decision autonomous inference.

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferencePermission {
    version: String,
    permission_id: Uuid,
    engagement_id: EngagementId,
    campaign_id: CampaignId,
    operator_ref: OperatorRef,
    authority_ref: AuthorityRef,
    authority_revision: u64,
    mission_revision: u64,
    goal_ref: GoalRef,
    exercise_mode: ExerciseMode,
    recipient: String,
    profile: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    execution_mode: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    run_profile: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    live_binding: Option<LiveBinding>,
    field_classes: Vec<SafeFieldClass>,
    starts_at: i64,
    ends_at: i64,
    provider_terms_ref: Uuid,
    provider_terms_approved: bool,
    campaign_limits: CampaignInferenceLimits,
    operation_limits: OperationInferenceLimits,
}

impl InferencePermission {
    fn check_profile(&self) -> Res<()> {
        if self.version != "m1_inference_permission_v6" && self.live_binding.is_some() {
            return Err(Fail::Input("inference_unsupported_profile"));
        }
        match self.version.as_str() {
            "m1_inference_permission_v6" => self.check_live(),
            "m1_inference_permission_v4" | "m1_inference_permission_v5" => self.check_successor(),
            "m1_inference_permission_v3" => self.check_autonomous(),
            _ => self.check_initial_profile(),
        }
    }

    fn check_field_classes(&self) -> Res<()> {
        let autonomous = self.autonomous_binding().is_some();
        let max = 8 + usize::from(autonomous);
        if !(1..=max).contains(&self.field_classes.len()) {
            return Err(Fail::Input("inference_invalid_field_classes"));
        }
        if [
            !autonomous,
            self.field_classes
                .contains(&SafeFieldClass::AdmittedCampaignDecisionFeedback),
        ]
        .into_iter()
        .all(|v| v)
        {
            return Err(Fail::Input("inference_invalid_field_classes"));
        }
        for (index, class) in self.field_classes.iter().enumerate() {
            if self.field_classes[..index].contains(class) {
                return Err(Fail::Input("inference_invalid_field_classes"));
            }
        }
        Ok(())
    }

    pub(crate) fn validate(&self, parent: &RegistrationFields) -> Res<()> {
        self.check_profile()?;
        if (
            self.operator_ref,
            self.authority_ref,
            self.authority_revision,
            self.goal_ref,
            self.exercise_mode,
        ) != (
            parent.operator_ref,
            parent.authority_ref,
            parent.authority_revision,
            parent.goal_ref,
            parent.exercise_mode,
        ) {
            return Err(Fail::Input("inference_basis_mismatch"));
        }
        if self.starts_at >= self.ends_at
            || self.starts_at < parent.starts_at()
            || self.ends_at > parent.ends_at()
        {
            return Err(Fail::Input("inference_invalid_window"));
        }
        self.campaign_limits.validate()?;
        self.check_limits()
    }

    fn validate_native_profile(&self) -> Res<()> {
        if self.version == "m1_inference_permission_v1" {
            return Ok(());
        }
        let o = &self.operation_limits;
        let fixed = [
            (o.json_depth, 32),
            (o.collection_entries, 64),
            (o.response_bytes, 65536),
            (o.proposal_bytes, 8192),
            (o.text_field_bytes, 1024),
            (o.repair_bytes, 2048),
            (o.repair_error_codes, 8),
            (o.connect_time_ms, 5000),
        ];
        if !self
            .field_classes
            .contains(&SafeFieldClass::AdmittedRepairFieldsCodes)
            || fixed.iter().any(|(actual, required)| actual != required)
        {
            return Err(Fail::Input("inference_unsupported_profile"));
        }
        Ok(())
    }

    fn envelope_depth(&self) -> i64 {
        if self.version != "m1_inference_permission_v1" {
            32
        } else {
            8
        }
    }

    pub(crate) fn bind_scope(&self, e: EngagementId, c: CampaignId, revision: u64) -> Res<()> {
        if self.engagement_id.0.is_nil()
            || self.campaign_id.0.is_nil()
            || (self.engagement_id, self.campaign_id, self.mission_revision) != (e, c, revision)
        {
            return Err(Fail::Input("inference_scope_mismatch"));
        }
        Ok(())
    }
}

impl InferencePermission {
    pub(crate) fn context_disclosure(&self) -> ContextDisclosure {
        let c = &self.campaign_limits;
        let o = &self.operation_limits;
        let output_token_capacity = o
            .calls_per_decision
            .checked_mul(o.output_tokens_per_call)
            .unwrap_or(0);
        ContextDisclosure {
            permission_id: self.permission_id,
            starts_at: self.starts_at,
            ends_at: self.ends_at,
            context_bytes: o.context_bytes as usize,
            configuration_bytes: o.configuration_bytes as usize,
            candidates: o.candidates as usize,
            required_classes: [
                SafeFieldClass::MissionQuestionConstraints,
                SafeFieldClass::ScopedOpaqueReferences,
                SafeFieldClass::OmissionsUnknown,
                SafeFieldClass::Capacity,
            ]
            .iter()
            .all(|c| self.field_classes.contains(c)),
            operator_premises: self
                .field_classes
                .contains(&SafeFieldClass::AdmittedOperatorPremises),
            owner_qualified_summaries: self
                .field_classes
                .contains(&SafeFieldClass::OwnerQualifiedSummaries),
            totals: [
                c.calls,
                c.request_bytes,
                c.response_bytes,
                c.output_token_capacity,
                c.wall_time_ms,
            ],
            required: [
                o.calls_per_decision,
                2 * o.request_bytes,
                2 * o.response_bytes,
                output_token_capacity,
                o.decision_wall_time_ms,
            ],
        }
    }
}

mod live;
pub use live::LiveBinding;
mod successor;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn context_disclosure_computes_selected_two_call_output_capacity() {
        let limits = json!({
            "context_bytes": 8192, "candidates": 8, "request_bytes": 16384,
            "configuration_bytes": 16384, "response_bytes": 65536, "calls_per_decision": 2,
            "output_tokens_per_call": 8192, "decision_wall_time_ms": 249000,
            "per_call_wall_time_ms": 122000, "soft_completion_ms": 120000,
            "shared_wall_time_ms": 244000, "phase_wall_time_ms": 254000,
            "cancel_grace_ms": 2000, "accounting_window_ms": 3000,
            "connect_time_ms": 5000, "proposal_bytes": 8192, "repair_bytes": 2048,
            "text_field_bytes": 1024, "repair_error_codes": 8, "json_depth": 32,
            "collection_entries": 64, "concurrency": 1
        });
        let mut raw = json!({
            "version": "m1_inference_permission_v5",
            "permission_id": Uuid::from_u128(1),
            "engagement_id": Uuid::from_u128(2),
            "campaign_id": Uuid::from_u128(3),
            "operator_ref": Uuid::from_u128(4),
            "authority_ref": Uuid::from_u128(5),
            "authority_revision": 1,
            "mission_revision": 1,
            "goal_ref": Uuid::from_u128(6),
            "exercise_mode": "blind",
            "recipient": "http://127.0.0.1:18083/responses",
            "profile": crate::m1_inference_profile::FIXTURE_PRO_V2_PROFILE,
            "run_profile": crate::m1_inference_profile::RUN_PROFILE,
            "execution_mode": "controlled_fixture",
            "field_classes": [
                "mission_question_constraints",
                "scoped_opaque_references",
                "owner_qualified_summaries",
                "admitted_operator_premises",
                "manifest_availability",
                "omissions_unknown",
                "capacity",
                "admitted_repair_fields_codes",
                "admitted_campaign_decision_feedback"
            ],
            "starts_at": 1700000001,
            "ends_at": 1700086399,
            "provider_terms_ref": Uuid::from_u128(7),
            "provider_terms_approved": true,
            "campaign_limits": {
                "calls": 10, "request_bytes": 100000, "response_bytes": 500000,
                "output_token_capacity": 32768, "wall_time_ms": 600000
            },
            "operation_limits": limits
        });
        let p_v2: InferencePermission = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(p_v2.context_disclosure().required[3], 16384);
        assert_eq!(p_v2.context_disclosure().required[4], 249000);
        raw["profile"] = json!("fixture-genai-responses-v1");
        raw["operation_limits"]["output_tokens_per_call"] = json!(4096);
        raw["operation_limits"]["decision_wall_time_ms"] = json!(129000);
        let p_v1: InferencePermission = serde_json::from_value(raw).unwrap();
        assert_eq!(p_v1.context_disclosure().required[3], 8192);
    }
}
