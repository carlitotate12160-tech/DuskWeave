//! Mission-owned inference capacity and operation-limit bounds.
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CampaignInferenceLimits {
    pub calls: i64,
    pub request_bytes: i64,
    pub response_bytes: i64,
    pub output_token_capacity: i64,
    pub wall_time_ms: i64,
}

impl CampaignInferenceLimits {
    pub(super) fn validate(&self) -> Res<()> {
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
/// consumption and uncertain accounting require the later Broker lane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OperationInferenceLimits {
    pub context_bytes: i64,
    pub candidates: i64,
    pub request_bytes: i64,
    pub configuration_bytes: i64,
    pub response_bytes: i64,
    pub calls_per_decision: i64,
    pub output_tokens_per_call: i64,
    pub decision_wall_time_ms: i64,
    pub connect_time_ms: i64,
    pub proposal_bytes: i64,
    pub repair_bytes: i64,
    pub text_field_bytes: i64,
    pub repair_error_codes: i64,
    pub json_depth: i64,
    pub collection_entries: i64,
    pub concurrency: i64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub per_call_wall_time_ms: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub soft_completion_ms: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub shared_wall_time_ms: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub phase_wall_time_ms: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub cancel_grace_ms: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub accounting_window_ms: Option<i64>,
}

impl OperationInferenceLimits {
    pub(super) fn validate(
        &self,
        depth: i64,
        context_cap: i64,
        output_cap: i64,
        decision_cap: i64,
    ) -> Res<()> {
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
        ) != (2, output_cap, 1)
            || self.connect_time_ms > self.decision_wall_time_ms
        {
            return Err(Fail::Input("inference_unsupported_operation_profile"));
        }
        Ok(())
    }

    pub(super) fn check_timers(&self, autonomous: bool) -> Res<()> {
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

// Absent extensions retain legacy serialization; explicit null never does.
pub(super) fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
