//! Actual PostgreSQL identity, privilege, catalog and atomicity counterexamples.
use duskweave::Fail;
use duskweave::mission::*;
use duskweave::postgres_mission::{PgMissionStore, qualify_runtime};
use duskweave::registration::{self, MissionStore};
use duskweave::withdrawal::WithdrawalReason;
use serde_json::{Value, json};
use uuid::Uuid;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_prepared_withdrawal.rs"]
mod prepared;
#[path = "support/m1_session.rs"]
mod session_support;
use prepared::*;

#[test]
fn actual_original_login_required_not_operator_role_or_guc() {
    let _g = db_support::db();
    let case = Case::new(0x7a01);
    let r = case.request();
    let before = case.snapshot();
    assert_eq!(
        submit(db_support::runtime_client(), &r, case.operation),
        Err(Fail::Config("unqualified_session_writer"))
    );
    assert_eq!(
        submit(session_support::other_broker_client(), &r, case.operation),
        Err(Fail::State("prepared_withdrawal_refused"))
    );
    let _grant = Grants::install(
        "GRANT dw_m1_broker_test TO dw_m1_broker_other WITH INHERIT FALSE, SET TRUE",
        "REVOKE dw_m1_broker_test FROM dw_m1_broker_other",
    );
    let mut b = session_support::other_broker_client();
    b.batch_execute("SET ROLE dw_m1_broker_test; SET duskweave.writer_oid='1'")
        .unwrap();
    assert_refused(raw(
        &mut b,
        &case,
        case.operation,
        &r,
        &serde_json::to_value(&case.source).unwrap(),
    ));
    let mut wrong_operator = r.clone();
    wrong_operator.withdrawal.operator_ref = OperatorRef(Uuid::from_u128(90));
    let mut wrong_generation = r.clone();
    wrong_generation.expected_session_generation += 1;
    let mut wrong_session = r.clone();
    wrong_session.session_operation_id = session_support::session_op();
    for bad in [wrong_operator, wrong_generation, wrong_session] {
        assert_refused(raw(
            &mut session_support::broker_client(),
            &case,
            case.operation,
            &bad,
            &serde_json::to_value(&case.source).unwrap(),
        ));
    }
    for source in [Value::Null, json!({}), {
        let mut s = serde_json::to_value(&case.source).unwrap();
        s["event_id"] = json!(Uuid::from_u128(90));
        s
    }] {
        assert_refused(raw(
            &mut session_support::broker_client(),
            &case,
            case.operation,
            &r,
            &source,
        ));
    }
    for mut client in [
        db_support::runtime_client(),
        session_support::broker_client(),
    ] {
        session_support::assert_fenced(client.execute(
            "INSERT INTO mission.withdrawals
             (engagement_id,campaign_id,operation_id,event_id,registration_operation_id,contract)
             VALUES($1,$2,$3,$4,$5,'{}')",
            &[
                &case.e.0,
                &case.c.0,
                &case.operation.0,
                &Uuid::from_u128(91),
                &case.source.operation_id.0,
            ],
        ));
    }
    assert_eq!(
        case.snapshot(),
        before,
        "denials must NOT mutate complete scoped catalogs"
    );
}

#[test]
fn registration_assessment_and_old_session_operation_collisions_are_denied() {
    let _g = db_support::db();
    let mut case = Case::new(0x7a02);
    let assessment = session_support::session_op();
    let planning = duskweave::planning::PlanningRequest {
        engagement_id: case.e,
        campaign_id: case.c,
        purpose_ref: GoalRef(Uuid::from_u128(0x13)),
        asset_ref: AssetRef(Uuid::from_u128(0x21)),
        expected_mission_revision: 1,
        current_authority_confirmed: false,
    };
    use duskweave::planning_assessment::PlanningStore;
    let (mut a, mut s, _) = db_support::ports();
    s.assess(&planning, assessment, false, &mut a).unwrap();
    let before = case.snapshot();
    for op in [case.source.operation_id, assessment] {
        assert_refused(raw(
            &mut session_support::broker_client(),
            &case,
            op,
            &case.request(),
            &serde_json::to_value(&case.source).unwrap(),
        ));
    }
    assert_eq!(case.snapshot(), before);
    let old_session = case.session;
    release(&case);
    case.session = session_support::session_op();
    case.generation = session_support::prepare(case.e, case.c, case.session)
        .unwrap()
        .unwrap()["generation"]
        .as_i64()
        .unwrap();
    let before = case.snapshot();
    assert_refused(raw(
        &mut session_support::broker_client(),
        &case,
        old_session,
        &case.request(),
        &serde_json::to_value(&case.source).unwrap(),
    ));
    assert_eq!(case.snapshot(), before);
}

#[test]
fn identical_replay_and_read_only_recovery_preserve_full_original_records() {
    let _g = db_support::db();
    let case = Case::new(0x7a03);
    let original = case.submit().unwrap().unwrap();
    let before = case.snapshot();
    assert_eq!(case.submit().unwrap().unwrap(), original);
    assert_eq!(case.recover().unwrap().unwrap(), original);
    assert_eq!(case.snapshot(), before);
    assert_eq!(
        submit(
            session_support::other_broker_client(),
            &case.request(),
            case.operation
        ),
        Err(Fail::State("prepared_withdrawal_refused"))
    );
    let mut changed = case.request();
    changed.withdrawal.reason = WithdrawalReason::ScopeConcern;
    assert_eq!(
        submit(session_support::broker_client(), &changed, case.operation),
        Err(Fail::State("prepared_withdrawal_refused"))
    );
    assert_decode(recover(&changed, case.operation));
    changed = case.request();
    changed.expected_session_generation += 1;
    assert_decode(recover(&changed, case.operation));
    assert_eq!(case.snapshot(), before);
    release(&case);
    let after_release = case.snapshot();
    assert_eq!(
        case.recover().unwrap().unwrap(),
        original,
        "historical recovery confers no writer identity"
    );
    assert_eq!(
        case.submit().unwrap().unwrap(),
        original,
        "original replay never refills authority"
    );
    assert_eq!(case.snapshot(), after_release);
    assert!(session_support::prepare(case.e, case.c, session_support::session_op()).is_err());
    assert_eq!(case.snapshot(), after_release);
}

#[test]
fn owner_link_malformed_contract_and_actual_catalog_corruption_are_not_missing() {
    let _g = db_support::db();
    let case = Case::new(0x7a04);
    case.submit().unwrap();
    for table in ["mission.withdrawals", "execution.prepared_withdrawals"] {
        let _damage =
            ContractDamage::install(&case, table, |c| c["event_id"] = json!(Uuid::from_u128(92)));
        let before = case.snapshot();
        assert_decode(case.recover());
        assert_eq!(case.snapshot(), before);
    }
    {
        let _owner =
            ContractDamage::install(&case, "mission.withdrawals", |c| c["version"] = json!(0));
        let _link = ContractDamage::install(&case, "execution.prepared_withdrawals", |c| {
            c["version"] = json!(0)
        });
        let before = case.snapshot();
        assert_decode(case.recover());
        assert_eq!(case.snapshot(), before);
    }
    let mut admin = session_support::admin_db_client();
    let catalog: Uuid = admin
        .query_one(
            "SELECT event_id FROM mission.withdrawals WHERE engagement_id=$1 AND campaign_id=$2",
            &[&case.e.0, &case.c.0],
        )
        .unwrap()
        .get(0);
    admin
        .execute(
            "UPDATE mission.withdrawals SET event_id=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&case.e.0, &case.c.0, &Uuid::from_u128(93)],
        )
        .unwrap();
    let before = case.snapshot();
    let result = case.recover();
    assert_eq!(case.snapshot(), before);
    admin
        .execute(
            "UPDATE mission.withdrawals SET event_id=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&case.e.0, &case.c.0, &catalog],
        )
        .unwrap();
    assert_decode(result);
    let owner:Value=admin.query_one(
        "SELECT to_jsonb(w) FROM mission.withdrawals w WHERE engagement_id=$1 AND campaign_id=$2",
        &[&case.e.0,&case.c.0]).unwrap().get(0);
    admin
        .execute(
            "DELETE FROM mission.withdrawals WHERE engagement_id=$1 AND campaign_id=$2",
            &[&case.e.0, &case.c.0],
        )
        .unwrap();
    let before = case.snapshot();
    let result = case.recover();
    assert_eq!(case.snapshot(), before);
    // Restore the exact fixture owner row through the original authenticated Broker;
    // the existing immutable link permits only this precise row.
    session_support::broker_client().execute(
        "INSERT INTO mission.withdrawals SELECT (jsonb_populate_record(NULL::mission.withdrawals,$1)).*",
        &[&owner]).unwrap();
    assert_decode(result);
}

#[test]
fn overgranted_link_dml_is_denied_by_rls_and_truncate_trigger() {
    let _g = db_support::db();
    let case = Case::new(0x7a05);
    case.submit().unwrap();
    let before = case.snapshot();
    let _grants = Grants::install(
        "GRANT INSERT,UPDATE,DELETE,TRUNCATE ON execution.prepared_withdrawals TO dw_runtime",
        "REVOKE INSERT,UPDATE,DELETE,TRUNCATE ON execution.prepared_withdrawals FROM dw_runtime",
    );
    let mut client = session_support::broker_client();
    assert_eq!(
        qualify_runtime(&mut client),
        Err(Fail::Config("unqualified_runtime"))
    );
    let rejected = client
        .execute(
            "INSERT INTO execution.prepared_withdrawals VALUES($1,$2,$3,$4,99,$5,'{}')",
            &[
                &case.e.0,
                &case.c.0,
                &Uuid::from_u128(94),
                &case.session.0,
                &session_support::broker_oid(),
            ],
        )
        .unwrap_err();
    assert_eq!(
        rejected.code().map(postgres::error::SqlState::code),
        Some("42501")
    );
    assert_eq!(
        client
            .execute(
                "UPDATE execution.prepared_withdrawals SET generation=99 WHERE engagement_id=$1",
                &[&case.e.0]
            )
            .unwrap(),
        0
    );
    assert_eq!(
        client
            .execute(
                "DELETE FROM execution.prepared_withdrawals WHERE engagement_id=$1",
                &[&case.e.0]
            )
            .unwrap(),
        0
    );
    let rejected = client
        .batch_execute("TRUNCATE execution.prepared_withdrawals")
        .unwrap_err();
    assert_eq!(
        rejected.as_db_error().unwrap().message(),
        "session_table_immutable"
    );
    assert_eq!(case.snapshot(), before);
}

#[test]
fn set_reachable_trigger_authority_is_rejected_on_the_new_link() {
    let _g = db_support::db();
    let case = Case::new(0x7a06);
    session_support::ensure_priv_login();
    let mut client = session_support::priv_client();
    qualify_runtime(&mut client).unwrap();
    let _grants = Grants::install(
        "CREATE ROLE dw_pw_trigger_probe NOLOGIN;
         GRANT TRIGGER ON execution.prepared_withdrawals TO dw_pw_trigger_probe;
         GRANT dw_pw_trigger_probe TO dw_m1_broker_priv WITH INHERIT FALSE, SET TRUE",
        "REVOKE dw_pw_trigger_probe FROM dw_m1_broker_priv;
         REVOKE TRIGGER ON execution.prepared_withdrawals FROM dw_pw_trigger_probe;
         DROP ROLE dw_pw_trigger_probe",
    );
    let before = case.snapshot();
    assert_eq!(
        qualify_runtime(&mut client),
        Err(Fail::Config("unqualified_runtime"))
    );
    assert_eq!(
        submit(client, &case.request(), case.operation),
        Err(Fail::Config("unqualified_runtime"))
    );
    assert_eq!(case.snapshot(), before);
}

#[test]
fn fault_after_link_before_owner_insert_rolls_back_both_and_fence_epoch() {
    let _g = db_support::db();
    let case = Case::new(0x7a07);
    let before = case.snapshot();
    {
        let _fault = LinkFault::install(&case);
        assert_eq!(
            case.submit(),
            Err(Fail::State("prepared_withdrawal_refused"))
        );
        assert_eq!(case.snapshot(), before);
        assert_eq!(case.recover().unwrap(), None);
    }
    assert!(case.submit().unwrap().is_some());
}

#[test]
fn withdrawal_removes_authority_even_after_original_operating_window_expires() {
    let _g = db_support::db();
    session_support::ensure_broker_logins();
    let (e, c) = db_support::scope(0x7a08);
    let end = session_support::now() + 8;
    let mut value = session_support::registration(e, c, end - 8, 0);
    value["ends_at"] = json!(end);
    value["m1_permission"]["ends_at"] = json!(end);
    let input = duskweave::input::parse_register(value.to_string().as_bytes()).unwrap();
    let (mut a, mut s, mut t) = db_support::ports();
    let reg_op = registration::prepare_operation(&mut a).unwrap();
    registration::register(&mut a, &mut s, &mut t, reg_op, &input).unwrap();
    let session = session_support::session_op();
    session_support::prepare(e, c, session).unwrap();
    let case = Case {
        e,
        c,
        session,
        operation: session_support::session_op(),
        generation: 1,
        source: s.outbox_event(e, c, reg_op).unwrap().unwrap(),
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while session_support::now() < end {
        assert!(
            std::time::Instant::now() < deadline,
            "database-clock expiry barrier"
        );
        std::thread::yield_now();
    }
    let fence = session_support::fence(e, c);
    assert!(case.submit().unwrap().is_some());
    assert_eq!(session_support::fence(e, c), fence);
    assert_eq!(
        PgMissionStore::new(db_support::runtime_client())
            .mission_view(e, c)
            .unwrap()
            .unwrap()
            .revision,
        2
    );
}
