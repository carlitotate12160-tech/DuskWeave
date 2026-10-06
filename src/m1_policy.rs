use crate::mission::*;
use crate::withdrawal::MissionAuthorityWithdrawn;
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum NamePurpose {
    DiscoveryDisclosure { name: String },
    Contact { name: String, asset_ref: AssetRef },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M1PolicyQuery {
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub expected_mission_revision: u64,
    pub goal_ref: GoalRef,
    pub exercise_mode: ExerciseMode,
    pub purpose: NamePurpose,
}

impl M1PolicyQuery {
    pub fn validate(&self) -> Res<()> {
        let (name, asset) = match &self.purpose {
            NamePurpose::DiscoveryDisclosure { name } => (name, self.goal_ref.0),
            NamePurpose::Contact { name, asset_ref } => (name, asset_ref.0),
        };
        if [
            self.engagement_id.0,
            self.campaign_id.0,
            self.goal_ref.0,
            asset,
        ]
        .iter()
        .any(uuid::Uuid::is_nil)
        {
            return Err(Fail::Input("nil_reference"));
        }
        if self.expected_mission_revision == 0 || self.expected_mission_revision > i64::MAX as u64 {
            return Err(Fail::Input("invalid_revision"));
        }
        crate::m1_permission::validate_query_name(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyReason {
    AuthorityWithdrawn,
    StaleRevision,
    PurposeMismatch,
    ModeMismatch,
    M1PolicyAbsent,
    OutsideOperatingWindow,
    MatchesNamePolicy,
    NameNotPermitted,
    MissionMissing,
    InvalidInput,
    InvalidConfiguration,
    ContractDecode,
    StorageFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDisposition {
    PolicyMatch,
    PolicyNoMatch,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PolicyProvenance {
    pub registration_operation_id: OperationId,
    pub registration_event_id: EventId,
    pub effective_mission_revision: u64,
    pub policy_version: Option<u32>,
    pub authority_ref: AuthorityRef,
    pub authority_revision: u64,
    pub goal_ref: GoalRef,
    pub exercise_mode: ExerciseMode,
    pub mission_starts_at: i64,
    pub mission_ends_at: i64,
    pub policy_starts_at: Option<i64>,
    pub policy_ends_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct M1PolicyResult {
    check_kind: &'static str,
    pub result: PolicyDisposition,
    pub reason: PolicyReason,
    pub checked_at: Option<i64>,
    pub engagement_id: Option<EngagementId>,
    pub campaign_id: Option<CampaignId>,
    pub snapshot: Option<PolicyProvenance>,
    current_permission: bool,
    dispatch_granted: bool,
    acquisition_qualified: bool,
}

impl M1PolicyResult {
    pub fn unavailable(error: Fail, query: Option<&M1PolicyQuery>) -> Self {
        let reason = match error {
            Fail::Input(_) => PolicyReason::InvalidInput,
            Fail::Config(_) => PolicyReason::InvalidConfiguration,
            Fail::Store("contract_decode") => PolicyReason::ContractDecode,
            _ => PolicyReason::StorageFailure,
        };
        Self {
            check_kind: "m1_name_policy_snapshot_v1",
            result: PolicyDisposition::Unavailable,
            reason,
            checked_at: None,
            engagement_id: query.map(|q| q.engagement_id),
            campaign_id: query.map(|q| q.campaign_id),
            snapshot: None,
            current_permission: false,
            dispatch_granted: false,
            acquisition_qualified: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct M1PolicySnapshot {
    engagement_id: EngagementId,
    campaign_id: CampaignId,
    registration: Option<MissionRegistered>,
    withdrawal: Option<MissionAuthorityWithdrawn>,
    checked_at: i64,
}

impl M1PolicySnapshot {
    pub fn new(
        e: EngagementId,
        c: CampaignId,
        registration: Option<MissionRegistered>,
        withdrawal: Option<MissionAuthorityWithdrawn>,
        checked_at: i64,
    ) -> Res<Self> {
        if let Some(event) = &registration {
            Self::check_registration(event, e, c)?;
        }
        if let Some(marker) = &withdrawal {
            marker
                .validate()
                .map_err(|_| Fail::Store("contract_decode"))?;
            let event = registration
                .as_ref()
                .ok_or(Fail::Store("contract_decode"))?;
            if (
                marker.request.engagement_id,
                marker.request.campaign_id,
                marker.registration_operation_id,
            ) != (e, c, event.operation_id)
            {
                return Err(Fail::Store("contract_decode"));
            }
        }
        Ok(Self {
            engagement_id: e,
            campaign_id: c,
            registration,
            withdrawal,
            checked_at,
        })
    }

    fn check_registration(event: &MissionRegistered, e: EngagementId, c: CampaignId) -> Res<()> {
        crate::trajectory::check_event(event).map_err(|_| Fail::Store("contract_decode"))?;
        if (event.engagement_id, event.campaign_id) != (e, c)
            || [e.0, c.0, event.event_id.0, event.operation_id.0]
                .iter()
                .any(uuid::Uuid::is_nil)
        {
            return Err(Fail::Store("contract_decode"));
        }
        Ok(())
    }

    fn reason(&self, query: &M1PolicyQuery, event: &MissionRegistered) -> PolicyReason {
        if self.withdrawal.is_some() {
            return PolicyReason::AuthorityWithdrawn;
        }
        if query.expected_mission_revision != event.owner_revision {
            return PolicyReason::StaleRevision;
        }
        if query.goal_ref != event.fields.goal_ref {
            return PolicyReason::PurposeMismatch;
        }
        if query.exercise_mode != event.fields.exercise_mode {
            return PolicyReason::ModeMismatch;
        }
        let Some(permission) = event.fields.m1_permission() else {
            return PolicyReason::M1PolicyAbsent;
        };
        if !(event.fields.starts_at() <= self.checked_at
            && self.checked_at < event.fields.ends_at())
        {
            return PolicyReason::OutsideOperatingWindow;
        }
        permission.query_reason(&query.purpose, self.checked_at)
    }

    fn provenance(&self, event: &MissionRegistered) -> PolicyProvenance {
        let summary = event
            .fields
            .m1_permission()
            .map(|p| p.summary(&event.fields));
        PolicyProvenance {
            registration_operation_id: event.operation_id,
            registration_event_id: event.event_id,
            effective_mission_revision: if self.withdrawal.is_some() {
                2
            } else {
                event.owner_revision
            },
            policy_version: summary.as_ref().map(|p| p.policy_version),
            authority_ref: event.fields.authority_ref,
            authority_revision: event.fields.authority_revision,
            goal_ref: event.fields.goal_ref,
            exercise_mode: event.fields.exercise_mode,
            mission_starts_at: event.fields.starts_at(),
            mission_ends_at: event.fields.ends_at(),
            policy_starts_at: summary.as_ref().map(|p| p.starts_at),
            policy_ends_at: summary.as_ref().map(|p| p.ends_at),
        }
    }
}

pub trait M1PolicyReader {
    fn read_policy_snapshot(&mut self, e: EngagementId, c: CampaignId) -> Res<M1PolicySnapshot>;
}

pub fn check_name_policy(
    reader: &mut impl M1PolicyReader,
    query: &M1PolicyQuery,
) -> Res<M1PolicyResult> {
    query.validate()?;
    let snapshot = reader.read_policy_snapshot(query.engagement_id, query.campaign_id)?;
    if (snapshot.engagement_id, snapshot.campaign_id) != (query.engagement_id, query.campaign_id) {
        return Err(Fail::Store("contract_decode"));
    }
    let mut result = M1PolicyResult::unavailable(Fail::State("mission_missing"), Some(query));
    result.checked_at = Some(snapshot.checked_at);
    result.reason = PolicyReason::MissionMissing;
    if let Some(event) = &snapshot.registration {
        result.reason = snapshot.reason(query, event);
        result.result = if result.reason == PolicyReason::MatchesNamePolicy {
            PolicyDisposition::PolicyMatch
        } else {
            PolicyDisposition::PolicyNoMatch
        };
        result.snapshot = Some(snapshot.provenance(event));
    }
    Ok(result)
}
