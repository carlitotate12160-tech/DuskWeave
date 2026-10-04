//! Convergence characterization of the registration -> assessment ->
//! history baseline. These scenarios pin the combined producer/consumer
//! behavior that the imported refactors and the remaining contract/label/
//! predecessor/append extractions must preserve: stable identities and
//! labels, ordered failure precedence, and byte-stable durable rows under
//! duplicate, recovery, republication and fixture corruption. They pass
//! unchanged before and after the refactor; the expired fixture window is
//! a bounded refusal, not positive admission.

use duskweave::mission::{CampaignId, EngagementId};
use duskweave::planning::NonpositiveDecision;
use duskweave::planning_assessment;
use duskweave::planning_history::{PlanningHistoryPort, read_decision};
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration::{self, OperationAllocator};
use duskweave::trajectory::Delivered;
use duskweave::{Fail, Res};
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Output};

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/planning_history_db.rs"]
mod history_support;
use db_support::*;
use history_support::*;

struct NeverAllocate;

impl OperationAllocator for NeverAllocate {
    fn allocate(&mut self) -> Res<uuid::Uuid> {
        panic!("duplicate/recovery and rejected paths must not allocate")
    }
}

/// Complete scoped rows of every producer/history table, so a rejected or
/// duplicate attempt proves no observable row change at all.
fn scoped_rows(table: &str, e: EngagementId, c: CampaignId) -> Vec<String> {
    let mut client = runtime_client();
    client
        .query(
            &format!(
                "SELECT row_to_json(t)::text FROM {table} t \
                 WHERE engagement_id=$1 AND campaign_id=$2 ORDER BY 1"
            ),
            &[&e.0, &c.0],
        )
        .unwrap()
        .iter()
        .map(|r| r.get::<_, String>(0))
        .collect()
}

fn scoped_state(e: EngagementId, c: CampaignId) -> Vec<Vec<String>> {
    [
        "mission.missions",
        "mission.registration_outbox",
        "mission.planning_assessments",
        "trajectory.registration_history",
        "trajectory.planning_history",
    ]
    .iter()
    .map(|table| scoped_rows(table, e, c))
    .collect()
}

fn registration_row(client: &mut postgres::Client, e: EngagementId, c: CampaignId) -> String {
    client
        .query_one(
            "SELECT row_to_json(t)::text FROM trajectory.registration_history t \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap()
        .get(0)
}

struct InputFile(PathBuf);

impl InputFile {
    fn new(label: &str, value: &Value) -> Self {
        let file = Self(std::env::temp_dir().join(format!("dw-m0conv-{label}.json")));
        file.write(value);
        file
    }
    fn write(&self, value: &Value) {
        std::fs::write(&self.0, value.to_string()).unwrap();
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for InputFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}

fn planning_history_cli(op: &str, file: &InputFile, recover: &str) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    cmd.env_clear();
    cmd.env(
        "DW_DATABASE_URL",
        std::env::var("DW_TEST_DATABASE_URL").unwrap(),
    );
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", profile);
    }
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    cmd.args([
        "planning-history",
        "--operation",
        op,
        "--input",
        file.path(),
        "--recover",
        recover,
    ])
    .output()
    .unwrap()
}

fn json_output(output: Output) -> Value {
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Register, assess, duplicate/recover, publish/repeat, then recover through
/// a fresh native CLI: identities, contract, labels and complete rows are
/// byte-stable and each owner/history records exactly one durable effect.
#[test]
fn combined_path_preserves_identity_labels_history_and_rows() {
    let _guard = db();
    let (e, c) = scope(0xc001);
    let (mut allocator, mut store, mut trajectory) = ports();
    let registration = accepted(&mut allocator, &mut store, &mut trajectory, e, c);
    let operation = registration::prepare_operation(&mut allocator).unwrap();
    let req = request(e, c);
    let ev = planning_assessment::assess(&mut allocator, &mut store, &req, operation, false)
        .unwrap()
        .expect("durable assessment in the registered scope");
    // The fixed fixture window is in the past: bounded refusal, and the
    // stored labels pin the scope/window guard result at assessment time.
    assert_eq!(ev.decision, NonpositiveDecision::RefusedExpired);
    assert_eq!(ev.assessment_labels(), ("matched", "expired"));
    assert_eq!(
        ev.basis.as_ref().unwrap().registration_operation_id,
        registration.operation_id
    );
    let produced = scoped_state(e, c);
    // Duplicate and producer-recovery return the original event and must
    // not allocate or change any durable row.
    assert_eq!(
        planning_assessment::assess(&mut NeverAllocate, &mut store, &req, operation, false)
            .unwrap(),
        Some(ev.clone())
    );
    assert_eq!(
        planning_assessment::assess(&mut NeverAllocate, &mut store, &req, operation, true).unwrap(),
        Some(ev.clone())
    );
    assert_eq!(scoped_state(e, c), produced);
    // Publish, then repeat: exactly one accepted row, no anomaly.
    assert_eq!(
        trajectory.inspect(&ev).unwrap(),
        Delivered::Unresolved("not_recorded")
    );
    assert_eq!(
        retry(|| trajectory.publish(&ev)).unwrap(),
        Delivered::Completed
    );
    let recorded = scoped_state(e, c);
    assert_eq!(trajectory.publish(&ev).unwrap(), Delivered::Duplicate);
    assert_eq!(scoped_state(e, c), recorded);
    assert_eq!(
        read_decision(&mut PgMissionStore::new(runtime_client()), &req, operation).unwrap(),
        Some(ev.clone())
    );
    // The accepted history row stores the complete contract unchanged.
    let row = runtime_client()
        .query_one(
            "SELECT contract, obligation, recorded_at=completed_at \
             FROM trajectory.planning_history \
             WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e.0, &c.0],
        )
        .unwrap();
    assert_eq!(row.get::<_, Value>(0), serde_json::to_value(&ev).unwrap());
    assert_eq!(row.get::<_, String>(1), "trajectory.planning_history.v1");
    assert!(row.get::<_, bool>(2));
    // A fresh native consumer process recovers the completed history view
    // with the original contract, labels and no current permission.
    let file = InputFile::new("request", &serde_json::to_value(&req).unwrap());
    let receipt = json_output(planning_history_cli(
        &operation.0.to_string(),
        &file,
        "true",
    ));
    assert_eq!(receipt["contract"], serde_json::to_value(&ev).unwrap());
    assert_eq!(receipt["history"], "completed");
    assert_eq!(receipt["history_source"], "trajectory");
    assert_eq!(receipt["complete_history"], true);
    assert_eq!(receipt["complete_assessment"], false);
    assert_eq!(receipt["current_permission"], false);
    assert_eq!(receipt["scope"], "matched");
    assert_eq!(receipt["window"], "expired");
    // One durable effect per owner and per history.
    assert_eq!(count("mission.missions", e, c), 1);
    assert_eq!(count("mission.registration_outbox", e, c), 1);
    assert_eq!(count("mission.planning_assessments", e, c), 1);
    assert_eq!(count("trajectory.registration_history", e, c), 1);
    effects(&ev, 1, 0);
}

/// A supported event carrying both an invalid header and an invalid typed
/// basis reports the header failure first; repairing only the header
/// exposes the unsupported basis. Owner/scope/causation stay valid so the
/// intended guard is the one reached.
#[test]
fn contract_header_failure_dominates_invalid_typed_basis() {
    let _guard = db();
    let ev = decision(0xc010, true);
    let mut trajectory = PgTrajectory::new(runtime_client());
    let mut bad = ev.clone();
    bad.producer = "other".into();
    bad.basis.as_mut().unwrap().revision = 0;
    assert_eq!(
        trajectory.publish(&bad),
        Err(Fail::Unresolved("unsupported_contract"))
    );
    bad.producer = "mission".into();
    assert_eq!(
        trajectory.publish(&bad),
        Err(Fail::Store("unsupported_basis"))
    );
    effects(&ev, 0, 0);
    // Control: the intact event still publishes to one accepted row.
    assert_eq!(
        retry(|| trajectory.publish(&ev)).unwrap(),
        Delivered::Completed
    );
    effects(&ev, 1, 0);
}

/// A registration-history row with a corrupted producer header and payload
/// yields unsupported_predecessor without new planning history; restoring
/// only the header exposes contract_decode, and restoring the exact
/// original row leaves publication usable.
#[test]
fn predecessor_header_dominates_payload_decode_then_restores() {
    let _guard = db();
    let ev = decision(0xc020, true);
    let (e, c) = (ev.engagement_id, ev.campaign_id);
    let mut trajectory = PgTrajectory::new(runtime_client());
    let mut administrator = admin();
    let intact = registration_row(&mut administrator, e, c);
    let original: Value = administrator
        .query_one(
            "SELECT contract FROM trajectory.registration_history \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap()
        .get(0);
    // Corrupt the producer header and payload together; the schema permits
    // these column changes and no constraint is relaxed.
    administrator
        .execute(
            "UPDATE trajectory.registration_history \
             SET producer='other', contract='{}'::jsonb \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap();
    let corrupted = registration_row(&mut administrator, e, c);
    assert_ne!(corrupted, intact);
    assert_eq!(
        trajectory.publish(&ev).unwrap(),
        Delivered::Unresolved("unsupported_predecessor")
    );
    effects(&ev, 0, 0);
    assert_eq!(registration_row(&mut administrator, e, c), corrupted);
    // Restoring only the header exposes the payload decode failure.
    administrator
        .execute(
            "UPDATE trajectory.registration_history SET producer='mission' \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap();
    assert_eq!(trajectory.publish(&ev), Err(Fail::Store("contract_decode")));
    effects(&ev, 0, 0);
    // Restoring the original payload exactly leaves the row byte-stable
    // and publication usable.
    administrator
        .execute(
            "UPDATE trajectory.registration_history SET contract=$3 \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0, &original],
        )
        .unwrap();
    assert_eq!(registration_row(&mut administrator, e, c), intact);
    assert_eq!(
        retry(|| trajectory.publish(&ev)).unwrap(),
        Delivered::Completed
    );
    effects(&ev, 1, 0);
}
