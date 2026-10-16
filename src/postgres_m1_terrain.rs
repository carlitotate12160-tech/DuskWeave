//! PostgreSQL adapter and Mission permission verification for M1 Terrain DNS.
use crate::m1_policy::{
    M1PolicyQuery, M1PolicyReader, NamePurpose, PolicyReason, check_name_policy,
};
use crate::m1_terrain::{
    CLAIM_KIND_DNS_IPV4, TerrainAcceptedEvent, TerrainClaimRevision, TerrainIds,
    evaluate_dns_observation,
};
use crate::m1_terrain_input::TerrainDnsRequest;
use crate::mission::{CampaignId, EngagementId, EventId, MissionRegistered, OperationId};
use crate::registration::MissionStore;
use crate::{Fail, Res};
use postgres::{Client, IsolationLevel, Transaction};
use serde_json::Value;

const QUALIFY_TERRAIN_SQL: &str = "SELECT pg_has_role(session_user,'dw_m1_terrain','MEMBER') \
AND NOT EXISTS(SELECT 1 FROM pg_roles r WHERE pg_has_role(session_user,r.oid,'SET') AND (r.rolsuper OR r.rolbypassrls OR r.rolcreatedb OR r.rolcreaterole)) \
AND NOT EXISTS(SELECT 1 FROM unnest(ARRAY['dw_runtime','dw_m1_broker','dw_m1_cognition','dw_m1_trajectory']) g WHERE pg_has_role(session_user,g,'MEMBER')) \
AND NOT EXISTS(SELECT 1 FROM pg_roles a WHERE pg_has_role(session_user,a.oid,'SET') AND (EXISTS(SELECT 1 FROM pg_namespace n WHERE n.nspname IN ('mission','execution','cognition','trajectory','terrain') AND (has_schema_privilege(a.oid,n.oid,'CREATE') OR pg_has_role(a.oid,n.nspowner,'MEMBER'))) OR EXISTS(SELECT 1 FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace WHERE t.relkind='r' AND ((n.nspname IN ('mission','execution','cognition','trajectory') AND (pg_has_role(a.oid,t.relowner,'MEMBER') OR has_table_privilege(a.oid,t.oid,'INSERT,UPDATE,DELETE,TRUNCATE,TRIGGER') OR has_any_column_privilege(a.oid,t.oid,'INSERT,UPDATE'))) OR (n.nspname='terrain' AND (pg_has_role(a.oid,t.relowner,'MEMBER') OR has_table_privilege(a.oid,t.oid,'DELETE,TRUNCATE,TRIGGER') OR (t.relname<>'current_claims' AND (has_table_privilege(a.oid,t.oid,'UPDATE') OR has_any_column_privilege(a.oid,t.oid,'UPDATE'))))))))) \
AND current_setting('fsync')='on' AND current_setting('full_page_writes')='on' AND current_setting('synchronous_commit')='on'";

const READ_EXISTING_SQL: &str = "SELECT o.request_hash, o.contract, cc.payload, \
floor(extract(epoch FROM statement_timestamp()))::bigint, \
(o.event_id = (o.contract->>'event_id')::uuid AND o.observation_id = ob.observation_id \
 AND o.material_id = m.material_id AND o.evaluation_id = ev.evaluation_id \
 AND o.claim_id = cr.claim_id AND o.claim_revision = cr.claim_revision \
 AND ob.request_hash = o.request_hash AND ob.payload = (o.contract->'observation') \
 AND m.payload = (o.contract->'material') AND ev.payload = (o.contract->'evaluation') \
 AND cr.payload = (o.contract->'claim') AND cc.claim_revision >= cr.claim_revision), \
(EXISTS(SELECT 1 FROM mission.registration_outbox r WHERE r.engagement_id = o.engagement_id \
  AND r.campaign_id = o.campaign_id AND r.operation_id = (o.contract->>'registration_operation_id')::uuid \
  AND r.event_id = (o.contract->>'registration_event_id')::uuid) \
 AND NOT EXISTS(SELECT 1 FROM mission.withdrawals w WHERE w.engagement_id = o.engagement_id AND w.campaign_id = o.campaign_id)) \
FROM terrain.outbox o \
LEFT JOIN terrain.observations ob ON ob.engagement_id = o.engagement_id AND ob.campaign_id = o.campaign_id AND ob.operation_id = o.operation_id \
LEFT JOIN terrain.materials m ON m.engagement_id = o.engagement_id AND m.campaign_id = o.campaign_id AND m.operation_id = o.operation_id \
LEFT JOIN terrain.evaluations ev ON ev.engagement_id = o.engagement_id AND ev.campaign_id = o.campaign_id AND ev.operation_id = o.operation_id \
LEFT JOIN terrain.claim_revisions cr ON cr.engagement_id = o.engagement_id AND cr.campaign_id = o.campaign_id AND cr.operation_id = o.operation_id \
LEFT JOIN terrain.current_claims cc ON cc.engagement_id = o.engagement_id AND cc.campaign_id = o.campaign_id \
  AND cc.subject_name = cr.subject_name AND cc.claim_kind = cr.claim_kind \
  AND cc.vantage_ref = cr.vantage_ref AND cc.resolver_ipv4 = cr.resolver_ipv4 \
WHERE o.engagement_id = $1 AND o.campaign_id = $2 AND o.operation_id = $3";

fn map_policy_reason(reason: PolicyReason) -> Result<(), &'static str> {
    match reason {
        PolicyReason::MatchesNamePolicy | PolicyReason::NameNotPermitted => Ok(()),
        PolicyReason::MissionMissing => Err("mission_missing"),
        PolicyReason::AuthorityWithdrawn => Err("authority_withdrawn"),
        PolicyReason::StaleRevision => Err("stale_revision"),
        PolicyReason::PurposeMismatch => Err("purpose_mismatch"),
        PolicyReason::ModeMismatch => Err("mode_mismatch"),
        PolicyReason::M1PolicyAbsent => Err("m1_policy_absent"),
        PolicyReason::OutsideOperatingWindow => Err("outside_operating_window"),
        _ => Err("name_not_permitted"),
    }
}

fn check_m1_bindings(
    reg: &MissionRegistered,
    purpose: &NamePurpose,
    req: &TerrainDnsRequest,
    now: i64,
    matched_name: bool,
) -> Option<&'static str> {
    if reg.fields.operator_ref != req.operator_ref {
        return Some("purpose_mismatch");
    }
    let Some(perm) = reg.fields.m1_permission() else {
        return Some("m1_policy_absent");
    };
    let (vantage, resolver) = perm.dns_vantage();
    if vantage != req.vantage_ref {
        return Some("vantage_mismatch");
    }
    if resolver.to_string() != req.resolver_ipv4 {
        return Some("resolver_mismatch");
    }
    let asset_ok = req.asset_ref.is_none_or(|a| {
        [
            reg.fields.included_assets.contains(&a),
            !reg.fields.excluded_assets.contains(&a),
        ]
        .into_iter()
        .all(|b| b)
    });
    if !asset_ok {
        return Some("asset_not_permitted");
    }
    if !matched_name {
        return Some("name_not_permitted");
    }
    let win = [
        req.perceived_at <= now,
        (reg.fields.starts_at()..reg.fields.ends_at()).contains(&req.perceived_at),
        perm.query_reason(purpose, req.perceived_at) == PolicyReason::MatchesNamePolicy,
    ];
    (!win.into_iter().all(|b| b)).then_some("outside_operating_window")
}

pub fn verify_mission_permission(
    reader: &mut (impl M1PolicyReader + MissionStore),
    req: &TerrainDnsRequest,
) -> Res<Result<(OperationId, EventId, i64), &'static str>> {
    req.validate()?;
    let name = req.subject_name.clone();
    let purpose = match req.asset_ref {
        Some(asset_ref) => NamePurpose::Contact { name, asset_ref },
        None => NamePurpose::DiscoveryDisclosure { name },
    };
    let query = M1PolicyQuery {
        engagement_id: req.engagement_id,
        campaign_id: req.campaign_id,
        expected_mission_revision: u64::try_from(req.expected_mission_revision)
            .map_err(|_| Fail::Input("invalid_bounds"))?,
        goal_ref: req.goal_ref,
        exercise_mode: req.exercise_mode,
        purpose: purpose.clone(),
    };
    let res = check_name_policy(reader, &query)?;
    if let Err(code) = map_policy_reason(res.reason) {
        return Ok(Err(code));
    }
    let now = res.checked_at.ok_or(Fail::Store("contract_decode"))?;
    let prov = res
        .snapshot
        .as_ref()
        .ok_or(Fail::Store("contract_decode"))?;
    let reg = reader
        .outbox_event(
            req.engagement_id,
            req.campaign_id,
            prov.registration_operation_id,
        )?
        .ok_or(Fail::Store("contract_decode"))?;
    if reg.event_id != prov.registration_event_id {
        return Err(Fail::Store("contract_decode"));
    }
    let matched = res.reason == PolicyReason::MatchesNamePolicy;
    if let Some(code) = check_m1_bindings(&reg, &purpose, req, now, matched) {
        return Ok(Err(code));
    }
    Ok(Ok((reg.operation_id, reg.event_id, now)))
}

pub fn qualify_terrain(client: &mut Client) -> Res<()> {
    let ok: bool = client
        .query_one(QUALIFY_TERRAIN_SQL, &[])
        .map_err(|_| Fail::Config("unqualified_terrain_role"))?
        .try_get(0)
        .map_err(|_| Fail::Config("unqualified_terrain_role"))?;
    ok.then_some(())
        .ok_or(Fail::Config("unqualified_terrain_role"))
}

fn map_pg_err(e: &postgres::Error) -> Fail {
    match e.code().map(postgres::error::SqlState::code) {
        Some("40001" | "40P01") => Fail::Store("serialization_retry"),
        Some("42501") => Fail::Config("unqualified_terrain_role"),
        Some("P0001") => Fail::State("terrain_accept_refused"),
        _ => Fail::Store("terrain_store_error"),
    }
}

pub type TerrainDnsRecord = (TerrainAcceptedEvent, TerrainClaimRevision, i64, bool);

pub struct PgTerrainStore {
    client: Client,
}

impl PgTerrainStore {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    fn load_existing(
        tx: &mut Transaction<'_>,
        e: EngagementId,
        c: CampaignId,
        op: OperationId,
        hash: &str,
    ) -> Res<Option<TerrainDnsRecord>> {
        let Some(row) = tx
            .query_opt(READ_EXISTING_SQL, &[&e.0, &c.0, &op.0])
            .map_err(|err| map_pg_err(&err))?
        else {
            let foreign: bool = tx
                .query_one(
                    "SELECT EXISTS(SELECT 1 FROM terrain.outbox WHERE operation_id=$1)",
                    &[&op.0],
                )
                .map_err(|err| map_pg_err(&err))?
                .try_get(0)
                .map_err(|_| Fail::Store("contract_decode"))?;
            return (!foreign)
                .then_some(None)
                .ok_or(Fail::Conflict("operation_conflict"));
        };
        let intact: Option<bool> = row.try_get(4).map_err(|_| Fail::Store("contract_decode"))?;
        if intact != Some(true) {
            return Err(Fail::Store("contract_decode"));
        }
        let stored_hash: String = row.try_get(0).map_err(|_| Fail::Store("contract_decode"))?;
        let raw_ev: Value = row.try_get(1).map_err(|_| Fail::Store("contract_decode"))?;
        let raw_cur: Value = row.try_get(2).map_err(|_| Fail::Store("contract_decode"))?;
        let now: i64 = row.try_get(3).map_err(|_| Fail::Store("contract_decode"))?;
        let auth_db: bool = row.try_get(5).map_err(|_| Fail::Store("contract_decode"))?;
        let ev: TerrainAcceptedEvent =
            serde_json::from_value(raw_ev).map_err(|_| Fail::Store("contract_decode"))?;
        let cur: TerrainClaimRevision =
            serde_json::from_value(raw_cur).map_err(|_| Fail::Store("contract_decode"))?;
        ev.validate()?;
        let hash_ok = [stored_hash == hash, ev.request_hash == hash]
            .into_iter()
            .all(|b| b);
        if !hash_ok {
            return Err(Fail::Conflict("input_mismatch"));
        }
        Ok(Some((ev, cur, now, auth_db)))
    }

    fn read_current_claim(
        tx: &mut Transaction<'_>,
        req: &TerrainDnsRequest,
    ) -> Res<Option<TerrainClaimRevision>> {
        let row = tx
            .query_opt(
                "SELECT payload, (claim_id = (payload->>'claim_id')::uuid \
                 AND claim_revision = (payload->>'claim_revision')::integer) \
                 FROM terrain.current_claims WHERE engagement_id=$1 AND campaign_id=$2 \
                 AND subject_name=$3 AND claim_kind=$4 AND vantage_ref=$5 AND resolver_ipv4=$6 FOR UPDATE",
                &[
                    &req.engagement_id.0,
                    &req.campaign_id.0,
                    &req.subject_name,
                    &CLAIM_KIND_DNS_IPV4,
                    &req.vantage_ref,
                    &req.resolver_ipv4,
                ],
            )
            .map_err(|e| map_pg_err(&e))?;
        let Some(r) = row else {
            return Ok(None);
        };
        let intact: Option<bool> = r.try_get(1).map_err(|_| Fail::Store("contract_decode"))?;
        if intact != Some(true) {
            return Err(Fail::Store("contract_decode"));
        }
        let v: Value = r.try_get(0).map_err(|_| Fail::Store("contract_decode"))?;
        serde_json::from_value(v)
            .map(Some)
            .map_err(|_| Fail::Store("contract_decode"))
    }

    pub fn record_dns(
        &mut self,
        mission: &mut (impl M1PolicyReader + MissionStore),
        req: &TerrainDnsRequest,
        op: OperationId,
    ) -> Res<Result<TerrainDnsRecord, &'static str>> {
        req.validate()?;
        if op.0.is_nil() {
            return Err(Fail::Input("operation_id"));
        }
        let hash = req.request_hash()?;
        let perm = verify_mission_permission(mission, req)?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::Serializable)
            .start()
            .map_err(|e| map_pg_err(&e))?;
        if let Some((ev, cur, now, auth_db)) =
            Self::load_existing(&mut tx, req.engagement_id, req.campaign_id, op, &hash)?
        {
            tx.commit().map_err(|e| map_pg_err(&e))?;
            let auth_ok = [auth_db, perm.is_ok()].into_iter().all(|b| b);
            return Ok(Ok((ev, cur, now, auth_ok)));
        }
        let reg = match perm {
            Ok(lineage) => lineage,
            Err(reason) => return Ok(Err(reason)),
        };
        let prior = Self::read_current_claim(&mut tx, req)?;
        let ids = TerrainIds::derive(req.engagement_id, req.campaign_id, op)?;
        let event = evaluate_dns_observation(req, prior.as_ref(), ids, reg)?;
        let payload = serde_json::to_value(&event).map_err(|_| Fail::Store("contract_encode"))?;
        tx.query_one("SELECT terrain.accept_dns_observation($1)", &[&payload])
            .map_err(|e| map_pg_err(&e))?;
        tx.commit()
            .map_err(|_| Fail::Unresolved("terrain_commit_unknown"))?;
        Ok(Ok((event.clone(), event.claim, reg.2, true)))
    }

    pub fn inspect_dns(
        &mut self,
        mission: &mut (impl M1PolicyReader + MissionStore),
        req: &TerrainDnsRequest,
        op: OperationId,
    ) -> Res<Option<TerrainDnsRecord>> {
        req.validate()?;
        if op.0.is_nil() {
            return Err(Fail::Input("operation_id"));
        }
        let hash = req.request_hash()?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .map_err(|e| map_pg_err(&e))?;
        let Some((ev, cur, now, auth_db)) =
            Self::load_existing(&mut tx, req.engagement_id, req.campaign_id, op, &hash)?
        else {
            tx.commit().map_err(|e| map_pg_err(&e))?;
            return Ok(None);
        };
        tx.commit().map_err(|e| map_pg_err(&e))?;
        let snap_ok = verify_mission_permission(mission, req).is_ok_and(|r| r.is_ok());
        let auth_ok = [auth_db, snap_ok].into_iter().all(|b| b);
        Ok(Some((ev, cur, now, auth_ok)))
    }
}
