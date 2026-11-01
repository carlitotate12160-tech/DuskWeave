//! Schema-0030 withdrawal SQL regressions under the live fence.
use super::fixture::*;
use duskweave::m1_effect_control::authority::RestrictedWithdrawal;
use duskweave::mission::{CampaignId, EngagementId, OperationId, OperatorRef};
use duskweave::postgres_m1_effect::PgStopWriter;
use duskweave::withdrawal::{WithdrawalReason, WithdrawalRequest};
use serde_json::{Value, json};
use uuid::Uuid;

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_withdrawal_without_session_commits_durably_and_recovers() {
    assert!(worker(), "SQL assertions run only in the fixture worker");
    let (e, c, operator) = register_live(0x5a01, true);
    let op = Uuid::new_v4();
    let q = withdrawal_q(e, c, operator);
    // No session exists: NULL generation resolves nothing yet still commits
    // the same durable Mission withdrawal/event/history obligation.
    let contract =
        live_withdrawal(&q, op, None, false).unwrap_or_else(|e| panic!("{}", db_message(&e)));
    assert_eq!(contract["operation_id"], json!(op));
    assert_eq!(contract["request"], q);
    assert_eq!(contract["kind"], json!("mission_authority_withdrawn"));
    assert_eq!(contract["producer"], json!("mission"));
    assert_eq!(
        admin()
            .query_one(
                "SELECT count(*) FROM mission.withdrawals WHERE engagement_id=$1 AND campaign_id=$2",
                &[&e, &c]
            )
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    // Identical replay reconciles to the same committed contract, not a conflict.
    let replay = live_withdrawal(&q, op, None, false).unwrap();
    assert_eq!(replay, contract);
    // Recovery reads only: same contract twice, and absence on a fresh scope.
    for _ in 0..2 {
        let recovered = live_withdrawal(&q, op, None, true).unwrap();
        assert_eq!(recovered, contract);
    }
    let (e2, c2, operator2) = register_live(0x5a02, true);
    let absent = stop_writer()
        .query_one(
            "SELECT mission.live_stop_withdrawal($1,$2,NULL,true)",
            &[&withdrawal_q(e2, c2, operator2), &Uuid::new_v4()],
        )
        .unwrap()
        .get::<_, Option<Value>>(0);
    assert_eq!(absent, None);
    // Conflicting operation or request against the committed row is refused.
    let conflict_op = live_withdrawal(&q, Uuid::new_v4(), None, false).unwrap_err();
    assert!(
        db_message(&conflict_op).contains("live_withdrawal_conflict"),
        "{conflict_op}"
    );
    let mut changed = q.clone();
    changed["reason"] = json!("authorization_ended");
    let conflict_q = live_withdrawal(&changed, op, None, false).unwrap_err();
    assert!(
        db_message(&conflict_q).contains("live_withdrawal_conflict"),
        "{conflict_q}"
    );
    // Stale revision and malformed requests never commit.
    let mut stale = q.clone();
    stale["expected_mission_revision"] = json!(2);
    assert!(live_withdrawal(&stale, Uuid::new_v4(), None, false).is_err());
    // A non-live mission cannot be withdrawn through the live stop-writer.
    let (e3, c3, operator3) = register_live(0x5a03, false);
    let nonlive = live_withdrawal(
        &withdrawal_q(e3, c3, operator3),
        Uuid::new_v4(),
        None,
        false,
    )
    .unwrap_err();
    assert!(
        db_message(&nonlive).contains("live_withdrawal_refused"),
        "{nonlive}"
    );
    // The production adapter drives the same fenced path and decodes the event.
    let (e4, c4, operator4) = register_live(0x5a04, true);
    let typed = WithdrawalRequest {
        engagement_id: EngagementId(e4),
        campaign_id: CampaignId(c4),
        operator_ref: OperatorRef(operator4),
        expected_mission_revision: 1,
        reason: WithdrawalReason::OperatorRequested,
    };
    let mut writer = PgStopWriter::scoped(stop_writer());
    let event =
        RestrictedWithdrawal::withdraw(&mut writer, &typed, OperationId(Uuid::new_v4()), false)
            .unwrap()
            .expect("adapter returns the durable event");
    assert_eq!(event.request, typed);
    let mut writer = PgStopWriter::scoped(stop_writer());
    let conflict =
        RestrictedWithdrawal::withdraw(&mut writer, &typed, OperationId(Uuid::new_v4()), false);
    assert!(matches!(
        conflict,
        Err(duskweave::Fail::State("live_withdrawal_refused"))
    ));
}

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_withdrawal_resolves_session_and_leaves_no_execution_grant() {
    assert!(worker());
    let (e, c, operator) = register_live(0x5b01, true);
    let (run, generation, _result) = fabricate_live_run(e, c, true, true);
    let q = withdrawal_q(e, c, operator);
    // The fenced function resolves the current session itself and stops the run.
    let contract = live_withdrawal(&q, Uuid::new_v4(), None, false).unwrap_or_else(|e| panic!("{}", db_message(&e)));
    assert_eq!(contract["kind"], json!("mission_authority_withdrawn"));
    let status: String = admin()
        .query_one(
            "SELECT status FROM execution.autonomous_control WHERE run_id=$1",
            &[&run],
        )
        .unwrap()
        .get(0);
    assert_eq!(status, "stopped", "withdrawal stops the racing run");
    // No surviving execution grant: may_start on the stopped run is refused.
    assert!(
        broker()
            .query_one(
                "SELECT execution.live_effect_may_start($1,$2)",
                &[&run, &generation]
            )
            .is_err(),
        "stopped run cannot reach an execution grant"
    );
    // Reverse order: withdrawal first prevents any later reservation.
    let (e2, c2, operator2) = register_live(0x5b02, true);
    let (run2, _, _) = fabricate_live_run(e2, c2, false, true);
    live_withdrawal(
        &withdrawal_q(e2, c2, operator2),
        Uuid::new_v4(),
        None,
        false,
    )
    .unwrap();
    let refused = broker()
        .query_one("SELECT execution.live_effect_reserve($1)", &[&run2])
        .unwrap_err();
    assert!(db_message(&refused).contains("run_not_current"), "{refused}");
    assert_eq!(
        admin()
            .query_one(
                "SELECT count(*) FROM execution.live_effect_sessions WHERE run_id=$1",
                &[&run2]
            )
            .unwrap()
            .get::<_, i64>(0),
        0,
        "withdrawal prevents a surviving reservation"
    );
}

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_stop_writer_cannot_reach_effect_or_writer_tables() {
    assert!(worker());
    let (e, c, _operator) = register_live(0x5f01, true);
    // The stop role reads generation metadata but cannot manufacture sessions.
    let generation: Option<i64> = stop_writer()
        .query_one("SELECT execution.live_stop_generation($1,$2)", &[&e, &c])
        .unwrap()
        .get(0);
    assert_eq!(generation, None);
    assert_eq!(
        stop_writer()
            .batch_execute("INSERT INTO execution.live_effect_sessions DEFAULT VALUES")
            .unwrap_err()
            .code(),
        Some(&postgres::error::SqlState::INSUFFICIENT_PRIVILEGE)
    );
    assert!(
        stop_writer()
            .batch_execute("INSERT INTO mission.withdrawals DEFAULT VALUES")
            .is_err(),
        "the restrictive function is the only writer lane"
    );
}
