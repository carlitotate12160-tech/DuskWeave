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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CampaignInferenceLimits {
    calls: i64,
    request_bytes: i64,
    response_bytes: i64,
    output_token_capacity: i64,
    wall_time_ms: i64,
}

impl CampaignInferenceLimits {
    fn validate(&self) -> Res<()> {
        let totals = [
            self.calls,
            self.request_bytes,
            self.response_bytes,
            self.output_token_capacity,
            self.wall_time_ms,
        ];
        if totals.iter().any(|n| *n <= 0) {
            return Err(Fail::Input("inference_invalid_campaign_limits"));
        }
        Ok(())
    }
}

/// Every operation bound is explicit. These numbers declare ceilings;
// consumption and uncertain accounting require the later Broker lane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationInferenceLimits {
    context_bytes: i64,
    candidates: i64,
    request_bytes: i64,
    configuration_bytes: i64,
    response_bytes: i64,
    calls_per_decision: i64,
    output_tokens_per_call: i64,
    decision_wall_time_ms: i64,
    connect_time_ms: i64,
    proposal_bytes: i64,
    repair_bytes: i64,
    text_field_bytes: i64,
    repair_error_codes: i64,
    json_depth: i64,
    collection_entries: i64,
    concurrency: i64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    per_call_wall_time_ms: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    soft_completion_ms: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    shared_wall_time_ms: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    phase_wall_time_ms: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    cancel_grace_ms: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    accounting_window_ms: Option<i64>,
}

impl OperationInferenceLimits {
    fn validate(&self, depth: i64, context_cap: i64, decision_cap: i64) -> Res<()> {
        let bounded = [
            (self.context_bytes, context_cap),
            (self.candidates, 8),
            (self.request_bytes, 16384),
            (self.configuration_bytes, 16384),
            (self.response_bytes, 65536),
            (self.decision_wall_time_ms, decision_cap),
            (self.connect_time_ms, 5000),
            (self.proposal_bytes, 8192),
            (self.repair_bytes, 2048),
            (self.text_field_bytes, 1024),
            (self.repair_error_codes, 8),
            (self.json_depth, depth),
            (self.collection_entries, 64),
        ];
        if bounded.iter().any(|(n, cap)| *n <= 0 || n > cap) {
            return Err(Fail::Input("inference_invalid_operation_limits"));
        }
        if (
            self.calls_per_decision,
            self.output_tokens_per_call,
            self.concurrency,
        ) != (2, 4096, 1)
            || self.connect_time_ms > self.decision_wall_time_ms
        {
            return Err(Fail::Input("inference_unsupported_operation_profile"));
        }
        Ok(())
    }
}

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
        if matches!(
            self.version.as_str(),
            "m1_inference_permission_v4" | "m1_inference_permission_v5"
        ) {
            return self.check_successor();
        }
        if self.version == "m1_inference_permission_v3" {
            return self.check_autonomous();
        }
        self.check_legacy_profile()?;
        if self.permission_id.is_nil() || self.provider_terms_ref.is_nil() {
            return Err(Fail::Input("nil_reference"));
        }
        self.check_field_classes()?;
        self.validate_native_profile()
    }

    fn check_legacy_profile(&self) -> Res<()> {
        let supported = matches!(
            (self.version.as_str(), self.profile.as_str()),
            ("m1_inference_permission_v1", "deepseek-responses-v1")
                | (
                    "m1_inference_permission_v2",
                    "deepseek-responses-envelope32-v1"
                )
        );
        if !supported
            || self.recipient != "https://api.deepseek.com/responses"
            || !self.provider_terms_approved
        {
            return Err(Fail::Input("inference_unsupported_profile"));
        }
        let valid = [self.execution_mode.is_none(), self.run_profile.is_none()];
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or(Fail::Input("inference_unsupported_profile"))
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

/// Narrow Mission-owned disclosure projection; arrays use calls, request bytes,
/// response bytes, output-token capacity, and shared wall-time milliseconds.
pub(crate) struct ContextDisclosure {
    pub permission_id: Uuid,
    pub starts_at: i64,
    pub ends_at: i64,
    pub context_bytes: usize,
    pub configuration_bytes: usize,
    pub candidates: usize,
    pub required_classes: bool,
    pub operator_premises: bool,
    pub owner_qualified_summaries: bool,
    pub totals: [i64; 5],
    pub required: [i64; 5],
}
impl InferencePermission {
    pub(crate) fn context_disclosure(&self) -> ContextDisclosure {
        let c = &self.campaign_limits;
        let o = &self.operation_limits;
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
                2,
                2 * o.request_bytes,
                2 * o.response_bytes,
                8192,
                o.decision_wall_time_ms,
            ],
        }
    }
}

// Absent extensions retain legacy serialization; explicit null never does.
fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

impl OperationInferenceLimits {
    fn check_timers(&self, autonomous: bool) -> Res<()> {
        if self.soft_completion_ms.is_some() {
            return Err(Fail::Input("inference_unsupported_operation_profile"));
        }
        let actual = [
            self.per_call_wall_time_ms,
            self.shared_wall_time_ms,
            self.phase_wall_time_ms,
            self.cancel_grace_ms,
            self.accounting_window_ms,
        ];
        let expected = if autonomous {
            [
                Some(20000),
                Some(40000),
                Some(50000),
                Some(2000),
                Some(3000),
            ]
        } else {
            [None; 5]
        };
        if actual != expected || (autonomous && self.decision_wall_time_ms != 45000) {
            return Err(Fail::Input("inference_unsupported_operation_profile"));
        }
        Ok(())
    }
}
impl InferencePermission {
    fn check_autonomous(&self) -> Res<()> {
        let binding = (
            self.run_profile.as_deref(),
            self.execution_mode.as_deref(),
            self.profile.as_str(),
            self.recipient.as_str(),
        );
        let supported = matches!(
            binding,
            (
                Some("autonomous-dns-fixture-v1"),
                Some("controlled_fixture"),
                "deterministic-autonomous-v1",
                "urn:duskweave:fixture:inference:v1"
            ) | (
                Some("autonomous-dns-provider-fixture-v1"),
                Some("controlled_fixture"),
                "deepseek-responses-autonomous-v1",
                "https://api.deepseek.com/responses"
            ) | (
                Some("autonomous-dns-v1"),
                Some("live"),
                "deepseek-responses-autonomous-v1",
                "https://api.deepseek.com/responses"
            )
        );
        let valid = [
            supported,
            self.provider_terms_approved,
            !self.permission_id.is_nil(),
            !self.provider_terms_ref.is_nil(),
            self.field_classes
                .contains(&SafeFieldClass::AdmittedCampaignDecisionFeedback),
        ];
        if !valid.into_iter().all(|v| v) {
            return Err(Fail::Input("inference_unsupported_profile"));
        }
        self.check_field_classes()?;
        self.validate_native_profile()
    }
    /// Bounded configured disclosure only; the caller must still obtain current authority.
    pub fn autonomous_binding(&self) -> Option<(&str, &str)> {
        self.run_profile
            .as_deref()
            .zip(self.execution_mode.as_deref())
    }
}

mod successor;
