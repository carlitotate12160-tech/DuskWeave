//! CampaignTrajectory owner: sourced acceptance of the published Mission
//! contract. Never imports Mission's aggregate or the application layer.

use crate::mission::{CONTRACT_KIND, CONTRACT_VERSION, MissionRegistered, PRODUCER};
use crate::planning::PlanningAssessed;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivered {
    Completed,
    Duplicate,
    Anomaly,
    Unresolved(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryStatus {
    Completed,
    Pending,
    /// Integrity anomaly; blocks completion even if an accepted row exists.
    Anomaly,
}

fn declared(ev: &MissionRegistered) -> bool {
    ev.producer == PRODUCER
        && ev.kind == CONTRACT_KIND
        && matches!(
            (ev.version, ev.fields.m1_permission().is_some()),
            (CONTRACT_VERSION, false) | (2, true)
        )
        && ev.owner_revision == 1
}

fn scoped(ev: &MissionRegistered) -> bool {
    ev.affected_entity == ev.campaign_id
        && ev.causation_id == ev.operation_id
        && ev.correlation_id == ev.operation_id
}

pub fn check_event(ev: &MissionRegistered) -> Result<(), &'static str> {
    if !declared(ev) {
        return Err("unsupported_contract");
    }
    if !scoped(ev) {
        return Err("scope_violation");
    }
    if ev.version == 2
        && [
            ev.event_id.0,
            ev.operation_id.0,
            ev.engagement_id.0,
            ev.campaign_id.0,
        ]
        .iter()
        .any(uuid::Uuid::is_nil)
    {
        return Err("scope_violation");
    }
    ev.fields.validate().map_err(|_| "invalid_fields")
}

/// Version-2 bases carry the owner's scope snapshot; it must equal the
/// accepted predecessor's scope exactly. Version-1 bases keep the old rules.
fn scope_matches(registered: &MissionRegistered, assessed: &PlanningAssessed) -> bool {
    let Some(scope) = assessed
        .basis
        .as_ref()
        .and_then(|basis| basis.scope.as_ref())
    else {
        return true;
    };
    let fields = &registered.fields;
    scope.goal_ref == fields.goal_ref
        && scope.included_assets == fields.included_assets
        && scope.excluded_assets == fields.excluded_assets
}

pub fn check_planning_predecessor(
    registered: &MissionRegistered,
    assessed: &PlanningAssessed,
) -> Result<(), &'static str> {
    let basis = assessed.basis.as_ref().ok_or("unsupported_predecessor")?;
    check_event(registered).map_err(|_| "unsupported_predecessor")?;
    if [registered.event_id.0, registered.operation_id.0]
        .iter()
        .any(uuid::Uuid::is_nil)
    {
        return Err("unsupported_predecessor");
    }
    let source = (
        registered.engagement_id,
        registered.campaign_id,
        registered.operation_id,
        registered.owner_revision,
    );
    let required = (
        assessed.engagement_id,
        assessed.campaign_id,
        basis.registration_operation_id,
        basis.revision,
    );
    let bounds = (
        registered.fields.exercise_mode(),
        registered.fields.starts_at(),
        registered.fields.ends_at(),
    );
    if source != required || bounds != (basis.exercise_mode, basis.starts_at, basis.ends_at) {
        return Err("unsupported_predecessor");
    }
    if !scope_matches(registered, assessed) {
        return Err("unsupported_predecessor");
    }
    Ok(())
}
