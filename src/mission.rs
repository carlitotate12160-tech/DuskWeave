//! Mission owner: registration authority and its published event contract.
//! Construction is owner-controlled: fields are private and the only way a
//! Mission or RegistrationFields exists is through validated constructors.

use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! scoped_id {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);
        impl $name {
            pub fn parse(raw: &str) -> Option<Self> {
                Uuid::parse_str(raw).ok().filter(|u| !u.is_nil()).map(Self)
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    )+};
}

scoped_id!(
    EngagementId,
    CampaignId,
    OperationId,
    EventId,
    OperatorRef,
    AuthorityRef,
    GoalRef,
    AssetRef
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExerciseMode {
    Blind,
    DefenderInformed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistrationFields {
    pub(crate) operator_ref: OperatorRef,
    pub(crate) authority_ref: AuthorityRef,
    pub(crate) authority_revision: u64,
    pub(crate) goal_ref: GoalRef,
    pub(crate) included_assets: Vec<AssetRef>,
    pub(crate) excluded_assets: Vec<AssetRef>,
    pub(crate) exercise_mode: ExerciseMode,
    pub(crate) starts_at: i64,
    pub(crate) ends_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) m1_permission: Option<crate::m1_permission::M1Permission>,
}

impl RegistrationFields {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        operator_ref: OperatorRef,
        authority_ref: AuthorityRef,
        authority_revision: u64,
        goal_ref: GoalRef,
        included_assets: Vec<AssetRef>,
        excluded_assets: Vec<AssetRef>,
        exercise_mode: ExerciseMode,
        starts_at: i64,
        ends_at: i64,
        m1_permission: Option<crate::m1_permission::M1Permission>,
    ) -> Res<Self> {
        let mut f = Self {
            operator_ref,
            authority_ref,
            authority_revision,
            goal_ref,
            included_assets,
            excluded_assets,
            exercise_mode,
            starts_at,
            ends_at,
            m1_permission,
        };
        f.validate()?;
        f.included_assets.sort();
        f.included_assets.dedup();
        f.excluded_assets.sort();
        f.excluded_assets.dedup();
        Ok(f)
    }

    fn check_refs(&self) -> Res<()> {
        let nil_ref = self.operator_ref.0.is_nil()
            || self.authority_ref.0.is_nil()
            || self.goal_ref.0.is_nil()
            || self
                .included_assets
                .iter()
                .chain(&self.excluded_assets)
                .any(|a| a.0.is_nil());
        if nil_ref {
            return Err(Fail::Input("nil_reference"));
        }
        Ok(())
    }

    fn check_bounds(&self) -> Res<()> {
        if self.authority_revision == 0 || self.authority_revision > i64::MAX as u64 {
            return Err(Fail::Input("invalid_revision"));
        }
        if self.included_assets.is_empty() {
            return Err(Fail::Input("empty_inclusion"));
        }
        if self.included_assets.len() > 64 || self.excluded_assets.len() > 64 {
            return Err(Fail::Input("asset_limit"));
        }
        if self.starts_at >= self.ends_at {
            return Err(Fail::Input("invalid_window"));
        }
        Ok(())
    }

    pub(crate) fn validate(&self) -> Res<()> {
        self.check_refs()?;
        self.check_bounds()?;
        if let Some(permission) = &self.m1_permission {
            permission.validate(self)?;
        }
        Ok(())
    }

    pub fn m1_permission(&self) -> Option<&crate::m1_permission::M1Permission> {
        self.m1_permission.as_ref()
    }

    pub fn included_assets(&self) -> &[AssetRef] {
        &self.included_assets
    }
    pub fn exercise_mode(&self) -> ExerciseMode {
        self.exercise_mode
    }
    pub fn starts_at(&self) -> i64 {
        self.starts_at
    }
    pub fn ends_at(&self) -> i64 {
        self.ends_at
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegistrationInput {
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub fields: RegistrationFields,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionRegistered {
    pub event_id: EventId,
    pub operation_id: OperationId,
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub producer: String,
    pub affected_entity: CampaignId,
    pub owner_revision: u64,
    pub kind: String,
    pub version: u32,
    pub causation_id: OperationId,
    pub correlation_id: OperationId,
    pub occurred_at: i64,
    pub recorded_at: i64,
    pub fields: RegistrationFields,
}

pub const PRODUCER: &str = "mission";
pub const CONTRACT_KIND: &str = "mission_registered";
pub const CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct Mission {
    pub(crate) engagement_id: EngagementId,
    pub(crate) campaign_id: CampaignId,
    operation_id: OperationId,
    revision: u64,
}

impl Mission {
    pub fn register(
        input: &RegistrationInput,
        operation_id: OperationId,
        event_id: EventId,
        now_unix: i64,
    ) -> Res<(Self, MissionRegistered)> {
        if input.engagement_id.0.is_nil()
            || input.campaign_id.0.is_nil()
            || operation_id.0.is_nil()
            || event_id.0.is_nil()
        {
            return Err(Fail::Input("nil_identity"));
        }
        input.fields.validate()?;
        let mission = Mission {
            engagement_id: input.engagement_id,
            campaign_id: input.campaign_id,
            operation_id,
            revision: 1,
        };
        let event = MissionRegistered {
            event_id,
            operation_id,
            engagement_id: input.engagement_id,
            campaign_id: input.campaign_id,
            producer: PRODUCER.to_string(),
            affected_entity: input.campaign_id,
            owner_revision: 1,
            kind: CONTRACT_KIND.to_string(),
            version: if input.fields.m1_permission.is_some() {
                2
            } else {
                CONTRACT_VERSION
            },
            causation_id: operation_id,
            correlation_id: operation_id,
            occurred_at: now_unix,
            recorded_at: now_unix,
            fields: input.fields.clone(),
        };
        Ok((mission, event))
    }

    pub fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
}
