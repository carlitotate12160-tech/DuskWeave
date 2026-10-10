//! Real deferred-COMMIT classification and physical successful COMMIT reply loss.
use duskweave::Fail;
use duskweave::withdrawal::MissionAuthorityWithdrawn;
use serde_json::Value;
use std::time::Duration;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_prepared_withdrawal.rs"]
mod prepared;
#[path = "support/database_wait_proxy.rs"]
mod proxy;
#[path = "support/m1_session.rs"]
mod session_support;
#[path = "support/database_wait_db.rs"]
mod wait_db;
use prepared::*;

#[test]
fn unclassified_deferred_commit_failure_is_unknown_never_guard_rejection() {
    let _g = db_support::db();
    let case = Case::new(0x7c01);
    let before = case.snapshot();
    let _fault = wait_db::DeferredFault::install(
        session_support::admin_db_client(),
        &case.e.to_string(),
        "P0001",
    );
    assert_eq!(
        case.submit(),
        Err(Fail::Unresolved("prepared_withdrawal_unknown"))
    );
    assert_eq!(case.snapshot(), before);
}

#[test]
fn serialization_and_deadlock_commit_aborts_are_confirmed_with_atomic_rollback() {
    let _g = db_support::db();
    for (slot, state) in [(0x7c02, "40001"), (0x7c03, "40P01")] {
        let case = Case::new(slot);
        let before = case.snapshot();
        let _fault = wait_db::DeferredFault::install(
            session_support::admin_db_client(),
            &case.e.to_string(),
            state,
        );
        assert_eq!(case.submit(), Err(Fail::Store("serialization_retry")));
        assert_eq!(case.snapshot(), before);
        assert_eq!(case.recover().unwrap(), None);
    }
}

fn args(case: &Case, recover: bool, file: &str) -> Vec<String> {
    vec![
        "m1-session-withdraw".into(),
        "--operation".into(),
        case.operation.to_string(),
        "--input".into(),
        file.into(),
        "--recover".into(),
        recover.to_string(),
    ]
}
fn receipt(output: &std::process::Output) -> Value {
    serde_json::from_slice(output.stdout.split(|b| *b == b'\n').next().unwrap()).unwrap()
}
fn assert_flags(value: &Value) {
    for flag in [
        "current_permission",
        "dispatch_granted",
        "acquisition_qualified",
        "host_control_qualified",
    ] {
        assert_eq!(value[flag], false);
    }
}

#[test]
fn physical_commit_reply_loss_proves_owner_and_link_before_caller_failure_and_recovery() {
    let _g = db_support::db();
    for watchdog in [false, true] {
        let case = Case::new(0x7c10 + u128::from(watchdog));
        let original_fence = session_support::fence(case.e, case.c).unwrap();
        let original_history = case.snapshot()[8].clone();
        let file = wait_db::InputFile::new(&case.input(), &case.operation.to_string());
        let target = std::net::SocketAddr::from(([127, 0, 0, 1], session_support::db_port()));
        let relay = proxy::Relay::start(target, b"COMMIT");
        let mut cmd = wait_db::cli(
            &args(&case, false, file.path()),
            &session_support::broker_dsn(relay.port()),
        );
        let (child, started) = wait_db::spawn_piped(&mut cmd);
        assert!(
            relay.observed(Duration::from_secs(10)),
            "COMMIT request barrier missing"
        );
        assert!(
            relay.consumed(Duration::from_secs(10)),
            "successful CommandComplete COMMIT barrier missing"
        );
        // This is an independent restricted read, before caller failure and before recovery.
        let row = db_support::runtime_client()
            .query_one(
                "SELECT to_jsonb(p),to_jsonb(w) FROM execution.prepared_withdrawals p
             JOIN mission.withdrawals w USING(engagement_id,campaign_id,operation_id)
             WHERE p.engagement_id=$1 AND p.campaign_id=$2 AND p.operation_id=$3",
                &[&case.e.0, &case.c.0, &case.operation.0],
            )
            .unwrap();
        let link: Value = row.get(0);
        let owner: Value = row.get(1);
        assert_eq!(link["contract"], owner["contract"]);
        let event: MissionAuthorityWithdrawn =
            serde_json::from_value(owner["contract"].clone()).unwrap();
        event.validate().unwrap();
        assert_eq!(event.operation_id, case.operation);
        assert_eq!(event.request, case.request().withdrawal);
        assert_eq!(event.registration_operation_id, case.source.operation_id);
        assert_eq!(owner["event_id"], serde_json::json!(event.event_id));
        assert_eq!(
            owner["registration_operation_id"],
            serde_json::json!(case.source.operation_id)
        );
        assert_eq!(owner["producer"], "mission");
        assert_eq!(owner["owner_revision"], 2);
        assert_eq!(
            owner["publication_obligation"],
            "trajectory.withdrawal_history.v1"
        );
        assert_eq!(
            link["session_operation_id"],
            serde_json::json!(case.session)
        );
        assert_eq!(link["generation"], 1);
        let writer = link["writer_oid"].as_str().unwrap().parse::<u32>().unwrap();
        assert_eq!(writer, session_support::broker_oid());
        assert_eq!(
            session_support::fence(case.e, case.c).unwrap(),
            original_fence
        );
        let durable = case.snapshot();
        assert_eq!(durable[8], original_history);
        assert_eq!(durable[3].as_array().unwrap().len(), 1);
        assert_eq!(durable[9].as_array().unwrap().len(), 1);
        assert_eq!(
            durable[6],
            serde_json::json!([]),
            "history cannot run before owner acknowledgment"
        );
        let (output, _) = if watchdog {
            let output = wait_db::wait_bounded(
                child,
                wait_db::COMMAND_ENVELOPE + Duration::from_secs(5),
                started,
            );
            drop(relay);
            output
        } else {
            drop(relay);
            wait_db::wait_bounded(child, Duration::from_secs(15), started)
        };
        if watchdog {
            wait_db::expect_watchdog_stop(
                &output,
                started.elapsed(),
                wait_db::COMMAND_FLOOR,
                wait_db::COMMAND_ENVELOPE,
            );
        } else {
            assert_eq!(output.status.code(), Some(1));
            let r = receipt(&output);
            assert_eq!(r["result"], "unknown");
            assert_flags(&r);
            assert_eq!(r["operation"], serde_json::json!(case.operation));
        }
        assert_eq!(case.snapshot(), durable);
        let runtime = std::env::var("DW_TEST_DATABASE_URL").unwrap();
        let recovered = wait_db::run_bounded(
            &mut wait_db::cli(&args(&case, true, file.path()), &runtime),
            Duration::from_secs(15),
        )
        .0;
        assert!(recovered.status.success());
        let r = receipt(&recovered);
        assert_flags(&r);
        assert_eq!(r["result"], "durable");
        assert_eq!(r["history"], "pending");
        assert_eq!(r["contract"], owner["contract"]);
        assert_eq!(case.snapshot(), durable);
        let broker = session_support::broker_dsn(session_support::db_port());
        let completed = wait_db::run_bounded(
            &mut wait_db::cli(&args(&case, false, file.path()), &broker),
            Duration::from_secs(15),
        )
        .0;
        assert!(completed.status.success());
        let r = receipt(&completed);
        assert_flags(&r);
        assert_eq!(r["contract"], owner["contract"]);
        assert_eq!(r["complete_history"], true);
        let after = case.snapshot();
        let replay = wait_db::run_bounded(
            &mut wait_db::cli(&args(&case, false, file.path()), &broker),
            Duration::from_secs(15),
        )
        .0;
        assert!(replay.status.success());
        assert_eq!(receipt(&replay)["contract"], owner["contract"]);
        assert_eq!(case.snapshot(), after);
        assert_eq!(after[6].as_array().unwrap().len(), 1);
    }
}
