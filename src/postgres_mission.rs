//! Mission owner's PostgreSQL adapter. Queries only mission.* tables;
//! the registration and its outbox contract commit in one transaction.

use crate::mission::*;
use crate::registration::{CommitEffect, MissionStore, MissionView, OperationAllocator};
use crate::{Fail, Res};
use postgres::{Client, IsolationLevel};

fn store_err(e: &postgres::Error) -> Fail {
    use postgres::error::SqlState;
    match e.code() {
        Some(&SqlState::T_R_SERIALIZATION_FAILURE) | Some(&SqlState::T_R_DEADLOCK_DETECTED) => {
            Fail::Store("serialization_retry")
        }
        Some(&SqlState::UNIQUE_VIOLATION) => Fail::Conflict("duplicate_identity"),
        _ => Fail::Store("storage_error"),
    }
}

fn mode_str(mode: ExerciseMode) -> &'static str {
    match mode {
        ExerciseMode::Blind => "blind",
        ExerciseMode::DefenderInformed => "defender_informed",
    }
}

pub struct PgMissionStore {
    client: Client,
}

impl PgMissionStore {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

fn prior_event(tx: &mut postgres::Transaction, m: &Mission) -> Res<Option<MissionRegistered>> {
    let row = tx
        .query_opt(
            "SELECT contract FROM mission.registration_outbox \
             WHERE engagement_id = $1 AND campaign_id = $2 AND operation_id = $3",
            &[&m.engagement_id.0, &m.campaign_id.0, &m.operation_id().0],
        )
        .map_err(|e| store_err(&e))?;
    row.map(|r| serde_json::from_value::<MissionRegistered>(r.get(0)))
        .transpose()
        .map_err(|_| Fail::Store("contract_decode"))
}

fn scope_occupied(tx: &mut postgres::Transaction, m: &Mission) -> Res<bool> {
    Ok(tx
        .query_opt(
            "SELECT 1 FROM mission.missions WHERE engagement_id = $1 AND campaign_id = $2",
            &[&m.engagement_id.0, &m.campaign_id.0],
        )
        .map_err(|e| store_err(&e))?
        .is_some())
}

fn insert_pair(tx: &mut postgres::Transaction, m: &Mission, ev: &MissionRegistered) -> Res<()> {
    let contract = serde_json::to_value(ev).map_err(|_| Fail::Store("encode"))?;
    let included =
        serde_json::to_value(&ev.fields.included_assets).map_err(|_| Fail::Store("encode"))?;
    let excluded =
        serde_json::to_value(&ev.fields.excluded_assets).map_err(|_| Fail::Store("encode"))?;
    tx.execute(
        "INSERT INTO mission.missions \
         (engagement_id, campaign_id, operator_ref, authority_ref, authority_revision, \
          goal_ref, included_assets, excluded_assets, exercise_mode, starts_at, ends_at, \
          revision, operation_id) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        &[
            &m.engagement_id.0,
            &m.campaign_id.0,
            &ev.fields.operator_ref.0,
            &ev.fields.authority_ref.0,
            &(ev.fields.authority_revision as i64),
            &ev.fields.goal_ref.0,
            &included,
            &excluded,
            &mode_str(ev.fields.exercise_mode),
            &ev.fields.starts_at,
            &ev.fields.ends_at,
            &(m.revision() as i64),
            &m.operation_id().0,
        ],
    )
    .map_err(|e| store_err(&e))?;
    tx.execute(
        "INSERT INTO mission.registration_outbox \
         (engagement_id, campaign_id, operation_id, event_id, contract) \
         VALUES ($1,$2,$3,$4,$5)",
        &[
            &m.engagement_id.0,
            &m.campaign_id.0,
            &m.operation_id().0,
            &ev.event_id.0,
            &contract,
        ],
    )
    .map_err(|e| store_err(&e))?;
    Ok(())
}

fn commit_tx(
    tx: &mut postgres::Transaction,
    m: &Mission,
    ev: &MissionRegistered,
) -> Res<CommitEffect> {
    if let Some(stored) = prior_event(tx, m)? {
        return if stored.fields == ev.fields {
            Ok(CommitEffect::Existing(Box::new(stored)))
        } else {
            Err(Fail::Conflict("integrity_conflict"))
        };
    }
    if scope_occupied(tx, m)? {
        return Err(Fail::Conflict("already_registered"));
    }
    insert_pair(tx, m, ev)?;
    Ok(CommitEffect::Fresh)
}

impl MissionStore for PgMissionStore {
    fn commit_registration(&mut self, m: &Mission, ev: &MissionRegistered) -> Res<CommitEffect> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .map_err(|e| store_err(&e))?;
        let effect = commit_tx(&mut tx, m, ev)?;
        // A failed commit() means the outcome is unknowable: UNKNOWN, not
        // proven rollback. The caller must reconcile the stable identity.
        tx.commit().map_err(|_| Fail::Store("commit_unknown"))?;
        Ok(effect)
    }

    fn mission_view(&mut self, e: EngagementId, c: CampaignId) -> Res<Option<MissionView>> {
        let row = self
            .client
            .query_opt(
                "SELECT operation_id, revision, exercise_mode, starts_at, ends_at \
                 FROM mission.missions WHERE engagement_id = $1 AND campaign_id = $2",
                &[&e.0, &c.0],
            )
            .map_err(|e| store_err(&e))?;
        Ok(row.map(|r| MissionView {
            operation_id: OperationId(r.get(0)),
            revision: r.get::<_, i64>(1) as u64,
            exercise_mode: match r.get::<_, &str>(2) {
                "defender_informed" => ExerciseMode::DefenderInformed,
                _ => ExerciseMode::Blind,
            },
            starts_at: r.get(3),
            ends_at: r.get(4),
        }))
    }

    fn outbox_event(
        &mut self,
        e: EngagementId,
        c: CampaignId,
        op: OperationId,
    ) -> Res<Option<MissionRegistered>> {
        let row = self
            .client
            .query_opt(
                "SELECT contract FROM mission.registration_outbox \
                 WHERE engagement_id = $1 AND campaign_id = $2 AND operation_id = $3",
                &[&e.0, &c.0, &op.0],
            )
            .map_err(|e| store_err(&e))?;
        row.map(|r| serde_json::from_value::<MissionRegistered>(r.get(0)))
            .transpose()
            .map_err(|_| Fail::Store("contract_decode"))
    }
}

pub fn qualify_runtime(c: &mut Client) -> Res<()> {
    let ok: bool = c
        .query_one(
            "SELECT \
             current_setting('fsync') = 'on' \
                 AND current_setting('full_page_writes') = 'on' \
                 AND current_setting('synchronous_commit') = 'on' \
             AND NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = current_user \
                 AND (rolsuper OR rolbypassrls)) \
             AND NOT has_schema_privilege(current_user, 'mission', 'CREATE') \
                 AND NOT has_schema_privilege(current_user, 'trajectory', 'CREATE') \
             AND NOT EXISTS (SELECT 1 FROM pg_class cl \
                 JOIN pg_namespace n ON n.oid = cl.relnamespace \
                 WHERE n.nspname IN ('mission','trajectory') AND cl.relkind = 'r' \
                 AND (cl.relowner = \
                      (SELECT oid FROM pg_roles WHERE rolname = current_user) \
                     OR has_table_privilege(current_user, cl.oid, 'UPDATE') \
                     OR has_table_privilege(current_user, cl.oid, 'DELETE') \
                     OR has_table_privilege(current_user, cl.oid, 'TRUNCATE')))",
            &[],
        )
        .map_err(|e| store_err(&e))?
        .get(0);
    if ok {
        Ok(())
    } else {
        Err(Fail::Config("unqualified_runtime"))
    }
}

pub struct PgAllocator {
    client: Client,
}

impl PgAllocator {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl OperationAllocator for PgAllocator {
    fn allocate(&mut self) -> Res<uuid::Uuid> {
        let row = self
            .client
            .query_one("SELECT gen_random_uuid()", &[])
            .map_err(|e| store_err(&e))?;
        Ok(row.get(0))
    }
}
