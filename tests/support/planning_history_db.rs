//! Planning-history fixtures over the shared isolated PostgreSQL setup.
#![allow(dead_code)]

use crate::db_support::*;
use duskweave::mission::{AssetRef, CampaignId, EngagementId, GoalRef, OperationId};
use duskweave::planning::{PlanningAssessed, PlanningRequest};
use duskweave::planning_assessment;
use duskweave::registration::OperationAllocator;
use postgres::{Client, NoTls};
use uuid::Uuid;

pub fn request(e: EngagementId, c: CampaignId) -> PlanningRequest {
    PlanningRequest {
        engagement_id: e,
        campaign_id: c,
        purpose_ref: GoalRef(Uuid::from_u128(0x13)),
        asset_ref: AssetRef(Uuid::from_u128(0x21)),
        expected_mission_revision: 1,
        current_authority_confirmed: true,
    }
}

pub fn decision(slot: u128, registered: bool) -> PlanningAssessed {
    let (e, c) = scope(slot);
    let (mut allocator, mut store, mut trajectory) = ports();
    if registered {
        accepted(&mut allocator, &mut store, &mut trajectory, e, c);
    }
    let operation = OperationId(allocator.allocate().unwrap());
    planning_assessment::assess(&mut allocator, &mut store, &request(e, c), operation, false)
        .unwrap()
        .unwrap()
}

pub fn admin() -> Client {
    let mut config = dsn("DW_TEST_ADMIN_DATABASE_URL");
    config.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    config.connect(NoTls).unwrap()
}

pub fn effects(event: &PlanningAssessed, accepted: i64, anomalies: i64) {
    let (e, c) = (event.engagement_id, event.campaign_id);
    assert_eq!(count("mission.planning_assessments", e, c), 1);
    assert_eq!(
        count_where("trajectory.planning_history", "AND status='accepted'", e, c),
        accepted
    );
    assert_eq!(
        count_where(
            "trajectory.planning_history",
            "AND completed_at IS NOT NULL",
            e,
            c
        ),
        accepted
    );
    assert_eq!(
        count_where("trajectory.planning_history", "AND status='anomaly'", e, c),
        anomalies
    );
}
