use crate::planning::{MissionBasis, MissionScope, PlanningRequest};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanningDecision {
    UnresolvedMissionBasis,
    RefusedRevisionMismatch,
    UnresolvedAuthorityUnconfirmed,
    UnresolvedEvaluationIncomplete,
    RefusedPurposeMismatch,
    RefusedAssetExcluded,
    RefusedAssetUnknown,
    RefusedNotYetValid,
    RefusedExpired,
    RefusedAuthorityWithdrawn,
    Eligible,
}

impl PlanningDecision {
    /// Expected decision for a given contract version. Version 1 retains its
    /// original semantics (no scope/window evaluation); version 2 orders the
    /// scoped guards. Version 4 is qualified separately from the v2 guards.
    pub fn for_request(
        version: u32,
        request: &PlanningRequest,
        basis: Option<&MissionBasis>,
        evaluated_at: i64,
    ) -> Res<Self> {
        match version {
            1 => Ok(Self::v1(request, basis)),
            2 => Ok(Self::v2(request, basis, evaluated_at)),
            _ => Err(Fail::Unresolved("unsupported_contract")),
        }
    }

    fn v1(request: &PlanningRequest, basis: Option<&MissionBasis>) -> Self {
        match basis {
            None => Self::UnresolvedMissionBasis,
            Some(basis) if request.expected_mission_revision != basis.revision => {
                Self::RefusedRevisionMismatch
            }
            Some(_) if !request.current_authority_confirmed => Self::UnresolvedAuthorityUnconfirmed,
            Some(_) => Self::UnresolvedEvaluationIncomplete,
        }
    }

    fn v2(request: &PlanningRequest, basis: Option<&MissionBasis>, evaluated_at: i64) -> Self {
        let Some(basis) = basis else {
            return Self::UnresolvedMissionBasis;
        };
        if request.expected_mission_revision != basis.revision {
            return Self::RefusedRevisionMismatch;
        }
        if !request.current_authority_confirmed {
            return Self::UnresolvedAuthorityUnconfirmed;
        }
        let Some(scope) = &basis.scope else {
            // Unreachable for a validated v2 contract; fail nonpositive.
            return Self::UnresolvedMissionBasis;
        };
        Self::scope_window(request, scope, basis, evaluated_at)
    }

    fn scope_window(
        request: &PlanningRequest,
        scope: &MissionScope,
        basis: &MissionBasis,
        evaluated_at: i64,
    ) -> Self {
        if request.purpose_ref != scope.goal_ref {
            return Self::RefusedPurposeMismatch;
        }
        // Exclusion wins even when an asset is also included.
        if scope.excluded_assets.contains(&request.asset_ref) {
            return Self::RefusedAssetExcluded;
        }
        if !scope.included_assets.contains(&request.asset_ref) {
            return Self::RefusedAssetUnknown;
        }
        if evaluated_at < basis.starts_at {
            return Self::RefusedNotYetValid;
        }
        if evaluated_at >= basis.ends_at {
            return Self::RefusedExpired;
        }
        Self::UnresolvedEvaluationIncomplete
    }

    /// Historical (scope, window) result labels describing the original
    /// assessment time. Version 1 never evaluated scope or window; for v2 the
    /// labels follow which guards the stored decision reached.
    pub(super) fn labels(self, version: u32) -> (&'static str, &'static str) {
        match version {
            2 => self.v2_labels(),
            4 if self == Self::Eligible => ("matched", "within_window"),
            _ => ("not_evaluated", "not_evaluated"),
        }
    }

    fn v2_labels(self) -> (&'static str, &'static str) {
        use PlanningDecision as D;
        match self {
            D::RefusedPurposeMismatch => ("purpose_mismatch", "not_evaluated"),
            D::RefusedAssetExcluded => ("excluded", "not_evaluated"),
            D::RefusedAssetUnknown => ("unknown", "not_evaluated"),
            D::RefusedNotYetValid | D::RefusedExpired | D::UnresolvedEvaluationIncomplete => {
                ("matched", self.window_label())
            }
            _ => ("not_evaluated", "not_evaluated"),
        }
    }

    /// Historical window result for a scope-matched decision; every other
    /// decision keeps the not-evaluated label.
    fn window_label(self) -> &'static str {
        match self {
            Self::RefusedNotYetValid => "not_yet_valid",
            Self::RefusedExpired => "expired",
            Self::UnresolvedEvaluationIncomplete => "within_window",
            _ => "not_evaluated",
        }
    }
}
