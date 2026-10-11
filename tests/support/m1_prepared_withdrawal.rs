//! Serialized prepared-withdrawal fixtures; no credential literals or unbounded readers.
#![allow(dead_code)]
use crate::{db_support, session_support};
use duskweave::m1_prepared_withdrawal::{
    PreparedWithdrawalRecord, PreparedWithdrawalRequest, withdraw_prepared,
};
use duskweave::mission::*;
use duskweave::postgres_mission::{PgMissionStore, qualify_runtime};
use duskweave::postgres_prepared_withdrawal::PgPreparedWithdrawalStore;
use duskweave::{Fail, Res};
use postgres::{Client, GenericClient};
use serde_json::{Value, json};

#[derive(Clone)]
pub struct Case {
    pub e: EngagementId,
    pub c: CampaignId,
    pub session: OperationId,
    pub operation: OperationId,
    pub generation: i64,
    pub source: MissionRegistered,
}

impl Case {
    pub fn new(slot: u128) -> Self {
        session_support::ensure_broker_logins();
        let (e, c) = db_support::scope(slot);
        let reg = session_support::register_m1(e, c, 0);
        let session = session_support::session_op();
        let record = session_support::prepare(e, c, session).unwrap().unwrap();
        let source = db_support::ports()
            .1
            .outbox_event(e, c, reg.operation_id)
            .unwrap()
            .unwrap();
        Self {
            e,
            c,
            session,
            operation: session_support::session_op(),
            generation: record["generation"].as_i64().unwrap(),
            source,
        }
    }

    pub fn request(&self) -> PreparedWithdrawalRequest {
        PreparedWithdrawalRequest {
            withdrawal: session_support::withdraw_req(self.e, self.c),
            session_operation_id: self.session,
            expected_session_generation: self.generation,
        }
    }

    pub fn input(&self) -> Value {
        json!({"withdrawal":self.request().withdrawal,"session_operation_id":self.session,
            "expected_session_generation":self.generation})
    }

    pub fn submit(&self) -> Res<Option<PreparedWithdrawalRecord>> {
        submit(
            session_support::broker_client(),
            &self.request(),
            self.operation,
        )
    }

    pub fn recover(&self) -> Res<Option<PreparedWithdrawalRecord>> {
        recover(&self.request(), self.operation)
    }

    pub fn snapshot(&self) -> Vec<Value> {
        snapshot(self.e, self.c)
    }
}

use duskweave::registration::MissionStore;

pub fn submit(
    mut client: Client,
    r: &PreparedWithdrawalRequest,
    op: OperationId,
) -> Res<Option<PreparedWithdrawalRecord>> {
    qualify_runtime(&mut client)?;
    withdraw_prepared(
        &mut PgPreparedWithdrawalStore::new(client),
        Some(&mut PgMissionStore::new(db_support::runtime_client())),
        r,
        op,
        false,
    )
}

pub fn recover(
    r: &PreparedWithdrawalRequest,
    op: OperationId,
) -> Res<Option<PreparedWithdrawalRecord>> {
    let mut client = db_support::runtime_client();
    qualify_runtime(&mut client)?;
    withdraw_prepared(
        &mut PgPreparedWithdrawalStore::new(client),
        None::<&mut PgMissionStore>,
        r,
        op,
        true,
    )
}

pub fn raw(
    client: &mut impl GenericClient,
    case: &Case,
    op: OperationId,
    r: &PreparedWithdrawalRequest,
    source: &Value,
) -> Result<postgres::Row, postgres::Error> {
    let reason = serde_json::to_value(r.withdrawal.reason).unwrap();
    client.query_one(
        "SELECT to_jsonb(execution.withdraw_prepared_session($1,$2,$3,$4,$5,$6,$7,$8,$9))",
        &[
            &case.e.0,
            &case.c.0,
            &op.0,
            &r.session_operation_id.0,
            &r.expected_session_generation,
            &r.withdrawal.operator_ref.0,
            &(r.withdrawal.expected_mission_revision as i64),
            &reason.as_str().unwrap(),
            source,
        ],
    )
}

/// Full ordered scoped row payloads include their actual catalogs and timestamps.
pub fn snapshot(e: EngagementId, c: CampaignId) -> Vec<Value> {
    let mut client = db_support::runtime_client();
    [
        "mission.missions",
        "mission.registration_outbox",
        "mission.planning_assessments",
        "mission.withdrawals",
        "trajectory.registration_history",
        "trajectory.planning_history",
        "trajectory.withdrawal_history",
        "execution.session_fences",
        "execution.session_history",
        "execution.prepared_withdrawals",
    ]
    .iter()
    .map(|table| {
        client.query_one(&format!(
            "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text),'[]'::jsonb) \
             FROM {table} t WHERE engagement_id=$1 AND campaign_id=$2"),
            &[&e.0,&c.0]).unwrap().get(0)
    })
    .collect()
}

pub fn release(case: &Case) {
    use duskweave::m1_session::{SessionAction, session};
    let mut store =
        duskweave::postgres_m1_session::PgSessionStore::new(session_support::broker_client());
    session(
        &mut store,
        None::<&mut PgMissionStore>,
        SessionAction::Release,
        &session_support::req(case.e, case.c),
        case.session,
    )
    .unwrap();
}

/// Scope-bound fixture fault between the immutable link and Mission INSERT.
pub struct LinkFault(Client);
impl LinkFault {
    pub fn install(case: &Case) -> Self {
        let mut admin = session_support::admin_db_client();
        admin
            .batch_execute(&format!(
            "CREATE FUNCTION execution.dw_pw_link_fault() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='fixture_link_fault'; END $$;
             CREATE TRIGGER dw_pw_link_fault AFTER INSERT ON execution.prepared_withdrawals
             FOR EACH ROW WHEN (NEW.engagement_id='{}'::uuid)
             EXECUTE FUNCTION execution.dw_pw_link_fault();",case.e))
            .unwrap();
        Self(admin)
    }
}
impl Drop for LinkFault {
    fn drop(&mut self) {
        let _ = self.0.batch_execute(
            "DROP TRIGGER dw_pw_link_fault ON execution.prepared_withdrawals;
            DROP FUNCTION execution.dw_pw_link_fault();",
        );
    }
}

/// Scoped corruption of an existing owner/link contract, restored on assertion failure.
pub struct ContractDamage {
    admin: Client,
    table: &'static str,
    e: EngagementId,
    c: CampaignId,
    original: Value,
}
impl ContractDamage {
    pub fn install(case: &Case, table: &'static str, mutate: impl FnOnce(&mut Value)) -> Self {
        assert!(["mission.withdrawals", "execution.prepared_withdrawals"].contains(&table));
        let mut admin = session_support::admin_db_client();
        let original: Value = admin
            .query_one(
                &format!("SELECT contract FROM {table} WHERE engagement_id=$1 AND campaign_id=$2"),
                &[&case.e.0, &case.c.0],
            )
            .unwrap()
            .get(0);
        let mut value = original.clone();
        mutate(&mut value);
        admin
            .execute(
                &format!(
                    "UPDATE {table} SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2"
                ),
                &[&case.e.0, &case.c.0, &value],
            )
            .unwrap();
        Self {
            admin,
            table,
            e: case.e,
            c: case.c,
            original,
        }
    }
}
impl Drop for ContractDamage {
    fn drop(&mut self) {
        let _ = self.admin.execute(
            &format!(
                "UPDATE {} SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2",
                self.table
            ),
            &[&self.e.0, &self.c.0, &self.original],
        );
    }
}

/// RAII privilege changes: restoring fixture grants also runs during unwinding.
pub struct Grants {
    admin: Client,
    undo: String,
}
impl Grants {
    pub fn install(sql: &str, undo: &str) -> Self {
        let mut admin = session_support::admin_db_client();
        admin.batch_execute(sql).unwrap();
        Self {
            admin,
            undo: undo.into(),
        }
    }
}
impl Drop for Grants {
    fn drop(&mut self) {
        let _ = self.admin.batch_execute(&self.undo);
    }
}

pub fn assert_refused(result: Result<postgres::Row, postgres::Error>) {
    let error = result.expect_err("guarded withdrawal must refuse");
    assert_eq!(
        error.code().map(postgres::error::SqlState::code),
        Some("P0001")
    );
}

pub fn assert_decode(result: Res<Option<PreparedWithdrawalRecord>>) {
    assert_eq!(result, Err(Fail::Unresolved("prepared_withdrawal_decode")));
}
