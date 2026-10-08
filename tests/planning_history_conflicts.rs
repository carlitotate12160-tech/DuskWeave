//! Bridge-conflict regression: a distinct anomaly pair sharing only one
//! identity axis must be durably retained and dominate completion through
//! either axis, including after fresh process recovery.
use duskweave::mission::{CampaignId, EngagementId, OperationId};
use duskweave::planning::PlanningAssessed;
use duskweave::planning_history::{PlanningHistoryPort, history_view};
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration::OperationAllocator;
use duskweave::trajectory::Delivered;
use serde_json::Value;
use std::process::{Command, Output};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/planning_history_db.rs"]
mod history_support;
use db_support::*;
use history_support::*;

fn produced_pair(e: EngagementId, c: CampaignId) -> (PlanningAssessed, PlanningAssessed) {
    let (mut alloc, mut store, _t) = ports();
    let req = request(e, c);
    let op_a = OperationId(alloc.allocate().unwrap());
    let op_b = OperationId(alloc.allocate().unwrap());
    let first = duskweave::planning_assessment::assess(&mut alloc, &mut store, &req, op_a, false)
        .unwrap()
        .unwrap();
    let second = duskweave::planning_assessment::assess(&mut alloc, &mut store, &req, op_b, false)
        .unwrap()
        .unwrap();
    (first, second)
}

fn anomaly_pairs(e: EngagementId, c: CampaignId) -> Vec<(Uuid, Uuid)> {
    let mut pairs: Vec<(Uuid, Uuid)> = runtime_client()
        .query(
            "SELECT event_id, operation_id FROM trajectory.planning_history \
             WHERE engagement_id=$1 AND campaign_id=$2 AND status='anomaly'",
            &[&e.0, &c.0],
        )
        .unwrap()
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
    pairs.sort();
    pairs
}

fn sorted(mut pairs: Vec<(Uuid, Uuid)>) -> Vec<(Uuid, Uuid)> {
    pairs.sort();
    pairs
}

fn assert_anomaly_integrity(e: EngagementId, c: CampaignId, expected: usize) {
    let row = runtime_client()
        .query_one(
            "SELECT count(*), bool_and(contract IS NULL), bool_and(completed_at IS NULL), \
             bool_and(anomaly_category='conflicting_identity') \
             FROM trajectory.planning_history \
             WHERE engagement_id=$1 AND campaign_id=$2 AND status='anomaly'",
            &[&e.0, &c.0],
        )
        .unwrap();
    assert_eq!(row.get::<_, i64>(0), expected as i64);
    assert!(row.get::<_, bool>(1));
    assert!(row.get::<_, bool>(2));
    assert!(row.get::<_, bool>(3));
}

fn assert_accepted_unchanged(e: EngagementId, c: CampaignId, events: &[&PlanningAssessed]) {
    let mut contracts: Vec<Value> = runtime_client()
        .query(
            "SELECT contract FROM trajectory.planning_history \
             WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted' \
             ORDER BY event_id",
            &[&e.0, &c.0],
        )
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect();
    contracts.sort_by_key(|a| a.to_string());
    let mut expected: Vec<Value> = events
        .iter()
        .map(|ev| serde_json::to_value(ev).unwrap())
        .collect();
    expected.sort_by_key(|a| a.to_string());
    assert_eq!(contracts, expected);
}

fn cli_recover(e: EngagementId, event: &PlanningAssessed) -> Value {
    let path = std::env::temp_dir().join(format!("dw-b1b-conflict-{e}.json"));
    std::fs::write(&path, serde_json::to_string(&event.request).unwrap()).unwrap();
    let output: Output = Command::new(env!("CARGO_BIN_EXE_duskweave"))
        .env_remove("DW_DATABASE_URL_FILE")
        .env("DW_DATABASE_CONFIG_MODE", "env-local")
        .env(
            "DW_DATABASE_URL",
            std::env::var("DW_TEST_DATABASE_URL").unwrap(),
        )
        .args([
            "planning-history",
            "--operation",
            &event.operation_id.0.to_string(),
            "--input",
            path.to_str().unwrap(),
            "--recover",
            "true",
        ])
        .output()
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

fn assert_b_blocked(e: EngagementId, c: CampaignId, a: &PlanningAssessed, b: &PlanningAssessed) {
    assert_anomaly_integrity(e, c, 2);
    assert_accepted_unchanged(e, c, &[a, b]);
    let mut fresh = PgTrajectory::new(runtime_client());
    assert_eq!(fresh.inspect(b).unwrap(), Delivered::Anomaly);
    assert_eq!(fresh.inspect(a).unwrap(), Delivered::Anomaly);
    let receipt = cli_recover(e, b);
    assert_eq!(receipt["history"], "anomaly");
    assert_eq!(receipt["history_reason"], "conflicting_identity");
    assert_eq!(receipt["complete_history"], false);
    assert_eq!(receipt["current_permission"], false);
    assert_eq!(receipt["contract"], serde_json::to_value(b).unwrap());
    assert_anomaly_integrity(e, c, 2);
}

#[test]
fn shared_event_bridge_marker_is_retained_and_blocks_second_identity() {
    let _guard = db();
    let (e, c) = scope(0xb1d0);
    let (a, b) = produced_pair(e, c);
    let mut trajectory = PgTrajectory::new(runtime_client());
    assert_eq!(trajectory.publish(&a).unwrap(), Delivered::Completed);
    assert_eq!(trajectory.publish(&b).unwrap(), Delivered::Completed);

    let mut changed_content = a.clone();
    changed_content.request.purpose_ref.0 = Uuid::from_u128(0x991);
    changed_content.validate().unwrap();
    assert_eq!(
        trajectory.publish(&changed_content).unwrap(),
        Delivered::Anomaly
    );
    assert_eq!(
        anomaly_pairs(e, c),
        sorted(vec![(a.event_id.0, a.operation_id.0)])
    );

    // Distinct conflicting pair: shares A's event axis but carries B's operation.
    let mut bridge = a.clone();
    bridge.operation_id = b.operation_id;
    bridge.affected_entity = b.operation_id;
    bridge.causation_id = b.operation_id;
    bridge.correlation_id = b.operation_id;
    bridge.validate().unwrap();
    assert_eq!(trajectory.publish(&bridge).unwrap(), Delivered::Anomaly);
    assert_eq!(
        anomaly_pairs(e, c),
        sorted(vec![
            (a.event_id.0, a.operation_id.0),
            (a.event_id.0, b.operation_id.0)
        ])
    );
    assert_b_blocked(e, c, &a, &b);

    // Same-pair retry is idempotent; it adds no marker and loses no evidence.
    assert_eq!(trajectory.publish(&bridge).unwrap(), Delivered::Anomaly);
    let recovered = history_view(&mut PgTrajectory::new(runtime_client()), &b, true);
    assert_eq!((recovered.state, recovered.complete), ("anomaly", false));
    assert_b_blocked(e, c, &a, &b);
}

#[test]
fn shared_operation_bridge_marker_is_retained_and_blocks_second_identity() {
    let _guard = db();
    let (e, c) = scope(0xb1d1);
    let (a, b) = produced_pair(e, c);
    let mut trajectory = PgTrajectory::new(runtime_client());
    assert_eq!(trajectory.publish(&a).unwrap(), Delivered::Completed);
    assert_eq!(trajectory.publish(&b).unwrap(), Delivered::Completed);

    let mut changed_content = a.clone();
    changed_content.request.purpose_ref.0 = Uuid::from_u128(0x992);
    changed_content.validate().unwrap();
    assert_eq!(
        trajectory.publish(&changed_content).unwrap(),
        Delivered::Anomaly
    );

    // Distinct conflicting pair: shares the anomalous operation but carries B's event.
    let mut bridge = a.clone();
    bridge.event_id = b.event_id;
    bridge.request.purpose_ref.0 = Uuid::from_u128(0x993);
    bridge.validate().unwrap();
    assert_eq!(trajectory.publish(&bridge).unwrap(), Delivered::Anomaly);
    assert_eq!(
        anomaly_pairs(e, c),
        sorted(vec![
            (a.event_id.0, a.operation_id.0),
            (b.event_id.0, a.operation_id.0)
        ])
    );
    assert_b_blocked(e, c, &a, &b);

    // Identical identities under another campaign remain unaffected.
    let mut foreign = b.clone();
    let (e2, c2) = scope(0xb1d2);
    foreign.engagement_id = e2;
    foreign.campaign_id = c2;
    foreign.request.engagement_id = e2;
    foreign.request.campaign_id = c2;
    foreign.validate().unwrap();
    let mut other = PgTrajectory::new(runtime_client());
    assert_eq!(other.publish(&foreign).unwrap(), Delivered::Completed);
    assert_eq!(other.inspect(&foreign).unwrap(), Delivered::Completed);
    assert_eq!(count("trajectory.planning_history", e2, c2), 1);
    assert_eq!(count("mission.planning_assessments", e2, c2), 0);
    assert_b_blocked(e, c, &a, &b);
}
