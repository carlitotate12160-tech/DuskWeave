//! Physical successful COMMIT reply loss on the owned loopback PostgreSQL relay.
use duskweave::mission::*;
use duskweave::registration::{self, MissionStore, TrajectoryPort};
use duskweave::trajectory::HistoryStatus;
use serde_json::Value;
use std::time::Duration;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/database_wait_proxy.rs"]
mod proxy;
#[path = "support/database_wait_db.rs"]
mod wait_db;

fn inspect(e: EngagementId, c: CampaignId) -> std::process::Output {
    consent::cli(&[
        "inspect",
        "--engagement",
        &e.to_string(),
        "--campaign",
        &c.to_string(),
    ])
}

#[test]
fn producer_commit_ack_loss_precedes_history_and_fresh_process_recovery() {
    let _g = db_support::db();
    let (e, c) = db_support::scope(0xc101);
    let op = OperationId(consent::id());
    let raw = consent::registration(e, c);
    let file = wait_db::InputFile::new(&raw, &op.to_string());
    let cfg = db_support::dsn("DW_TEST_DATABASE_URL");
    let target = std::net::SocketAddr::from(([127, 0, 0, 1], *cfg.get_ports().first().unwrap()));
    let relay = proxy::Relay::start(target, b"COMMIT");
    let args = vec![
        "register".into(),
        "--operation".into(),
        op.to_string(),
        "--input".into(),
        file.path().into(),
    ];
    let (mut child, started) = wait_db::spawn_piped(&mut wait_db::cli(
        &args,
        &wait_db::relay_dsn(&cfg, relay.port()),
    ));
    assert!(
        relay.observed(Duration::from_secs(10)),
        "producer COMMIT request barrier"
    );
    assert!(
        relay.consumed(Duration::from_secs(10)),
        "producer CommandComplete COMMIT barrier"
    );
    assert!(
        child.try_wait().unwrap().is_none(),
        "durable proof must precede caller failure"
    );
    consent::assert_rows(e, c, 0);
    let (_, mut s, _) = db_support::ports();
    let original = s.outbox_event(e, c, op).unwrap().unwrap();
    assert_eq!(original.version, 3);
    assert_eq!(original.operation_id, op);
    let source:Value=db_support::runtime_client().query_one(
        "SELECT to_jsonb(o) FROM mission.registration_outbox o WHERE engagement_id=$1 AND campaign_id=$2",
        &[&e.0,&c.0]).unwrap().get(0);
    assert_eq!(source["event_id"], serde_json::json!(original.event_id));
    assert_eq!(source["operation_id"], serde_json::json!(op));
    assert_eq!(
        source["contract"]["fields"]["m1_inference_permission"],
        raw["m1_inference_permission"]
    );
    let owner: Value = db_support::runtime_client()
        .query_one(
            "SELECT to_jsonb(m) FROM mission.missions m WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap()
        .get(0);
    assert_eq!(owner["operation_id"], serde_json::json!(op));
    assert_eq!(owner["revision"], 1);
    assert_eq!(owner["campaign_id"], serde_json::json!(c));
    drop(relay);
    let output = wait_db::wait_bounded(child, Duration::from_secs(15), started).0;
    assert!(output.status.success());
    assert!(consent::text(&output).contains("result=unknown"));
    assert!(consent::text(&output).contains("reconcile_before_retry"));
    assert!(!consent::text(&output).contains("accepted"));
    consent::assert_rows(e, c, 0);
    let pending = inspect(e, c);
    assert!(pending.status.success());
    assert!(consent::text(&pending).contains("history.status=pending"));
    let recovered = consent::cli(&[
        "reconcile",
        "--engagement",
        &e.to_string(),
        "--campaign",
        &c.to_string(),
        "--operation",
        &op.to_string(),
    ]);
    assert!(recovered.status.success());
    let retry = consent::cli(&[
        "register",
        "--operation",
        &op.to_string(),
        "--input",
        file.path(),
    ]);
    assert!(retry.status.success());
    assert!(consent::text(&retry).contains(&format!("event={}", original.event_id)));
    assert_eq!(s.outbox_event(e, c, op).unwrap().unwrap(), original);
    consent::assert_rows(e, c, 1);
}

#[test]
fn consumer_commit_ack_loss_has_exact_durable_history_before_caller_failure() {
    let _g = db_support::db();
    let (e, c) = db_support::scope(0xc201);
    let op = OperationId(consent::id());
    let (mission, original) =
        Mission::register(&consent::input(e, c), op, EventId(consent::id()), 5).unwrap();
    let (_, mut s, _) = db_support::ports();
    s.commit_registration(&mission, &original).unwrap();
    let cfg = db_support::dsn("DW_TEST_DATABASE_URL");
    let target = std::net::SocketAddr::from(([127, 0, 0, 1], *cfg.get_ports().first().unwrap()));
    let relay = proxy::Relay::start(target, b"COMMIT");
    let args = vec![
        "reconcile".into(),
        "--engagement".into(),
        e.to_string(),
        "--campaign".into(),
        c.to_string(),
        "--operation".into(),
        op.to_string(),
    ];
    let (mut child, started) = wait_db::spawn_piped(&mut wait_db::cli(
        &args,
        &wait_db::relay_dsn(&cfg, relay.port()),
    ));
    assert!(
        relay.observed(Duration::from_secs(10)),
        "consumer COMMIT request barrier"
    );
    assert!(
        relay.consumed(Duration::from_secs(10)),
        "consumer CommandComplete COMMIT barrier"
    );
    assert!(
        child.try_wait().unwrap().is_none(),
        "durable proof must precede caller failure"
    );
    consent::assert_rows(e, c, 1);
    let row=db_support::runtime_client().query_one(
        "SELECT to_jsonb(h) FROM trajectory.registration_history h WHERE engagement_id=$1 AND campaign_id=$2 AND event_id=$3 AND status='accepted'",
        &[&e.0,&c.0,&original.event_id.0]).unwrap();
    let history: Value = row.get(0);
    assert_eq!(
        history["contract"],
        serde_json::to_value(&original).unwrap()
    );
    assert_eq!(history["operation_id"], serde_json::json!(op));
    assert_eq!(history["producer"], "mission");
    assert_eq!(history["obligation"], "trajectory.registration_history.v1");
    assert!(!history["completed_at"].is_null());
    let (_, _, mut fresh) = db_support::ports();
    assert_eq!(
        fresh.registration_status(&original).unwrap(),
        HistoryStatus::Completed
    );
    drop(relay);
    let output = wait_db::wait_bounded(child, Duration::from_secs(15), started).0;
    assert!(
        !output.status.success(),
        "lost reply cannot become caller completion"
    );
    let (_, mut s, mut t) = db_support::ports();
    assert_eq!(
        registration::reconcile(&mut s, &mut t, e, c, op).unwrap(),
        registration::ReconcileOutcome::Committed
    );
    let recovered = consent::cli(&[
        "reconcile",
        "--engagement",
        &e.to_string(),
        "--campaign",
        &c.to_string(),
        "--operation",
        &op.to_string(),
    ]);
    assert!(recovered.status.success());
    let inspected = inspect(e, c);
    assert!(inspected.status.success());
    assert!(consent::text(&inspected).contains("history.status=completed"));
    let durable:Value=db_support::runtime_client().query_one(
        "SELECT to_jsonb(h) FROM trajectory.registration_history h WHERE engagement_id=$1 AND campaign_id=$2 AND event_id=$3 AND status='accepted'",
        &[&e.0,&c.0,&original.event_id.0]).unwrap().get(0);
    assert_eq!(durable, history);
    assert_eq!(s.outbox_event(e, c, op).unwrap().unwrap(), original);
    consent::assert_rows(e, c, 1);
}
