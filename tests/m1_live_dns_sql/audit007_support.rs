//! Session/catalog observation and scoped snapshot support for the audit007
//! restricted-caller catalog-resolution cases. Test-local temporary objects
//! and safe observation values only; no production bypass, fixture
//! provisioner or runtime API. Only the audit007 module consumes this.
use super::fixture::{admin, broker, client, db_message};
use postgres::Client;
use postgres::types::ToSql;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use uuid::Uuid;

/// The exact installed functions whose search_path disposition is inspected.
/// Signature text is resolved through regprocedure; a missing or changed
/// signature is a packet dependency delta, not a skipped function.
pub(crate) const ALTERED_FUNCTIONS: [&str; 25] = [
    "execution.bind_inference_input(uuid,uuid,uuid,text)",
    "execution.close_fixture_session(uuid,text)",
    "execution.close_inference(uuid,uuid,uuid)",
    "execution.guard_mission_writer()",
    "execution.inference_call_state(uuid,text,jsonb)",
    "execution.inference_may_send(uuid,uuid,uuid,uuid,text,text,text,text)",
    "execution.inference_reservation(text,uuid,jsonb,jsonb)",
    "execution.inference_window_ms(uuid,uuid,uuid)",
    "execution.live_dns_result(uuid,bigint,jsonb)",
    "execution.live_effect_record(uuid)",
    "execution.live_effect_reserve(uuid)",
    "execution.live_effect_stop(uuid,bigint,boolean)",
    "execution.prepare_session(uuid,uuid,uuid,uuid,bigint,jsonb)",
    "execution.private_live_actor(text)",
    "execution.private_live_control_scope(uuid,text)",
    "execution.private_live_deadline(uuid)",
    "execution.private_live_mutation_guard()",
    "execution.release_prepared_session(uuid,uuid,uuid,uuid,bigint)",
    "execution.run_current(uuid)",
    "execution.run_record(uuid)",
    "execution.run_start(jsonb,text)",
    "execution.withdraw_prepared_session(uuid,uuid,uuid,uuid,bigint,uuid,bigint,text,jsonb)",
    "mission.live_stop_withdrawal(jsonb,uuid,bigint,boolean)",
    "terrain.admit_dns_request(jsonb)",
    "terrain.current_dns_context_view(jsonb)",
];

/// Restricted foreign Broker principal provisioned by the fixture bootstrap.
pub(crate) fn other_broker() -> Client {
    client("DW_NATIVE_OTHER_BROKER_DATABASE_URL")
}

/// Restricted Terrain principal whose column grant keeps it unqualified.
pub(crate) fn terrain_column() -> Client {
    client("DW_NATIVE_TERRAINCOLUMN_DATABASE_URL")
}

/// Safe session observation: caller identity, power flags, TEMP eligibility
/// and real memberships/SET eligibility through pg_catalog only.
pub(crate) fn session_metadata(c: &mut Client) -> Value {
    let row = c
        .query_one(
            "SELECT rolname,oid::bigint,rolsuper,rolbypassrls,rolcreatedb,rolcreaterole,\
             rolreplication,has_database_privilege(session_user,current_database(),'TEMP') \
             FROM pg_catalog.pg_roles WHERE rolname=session_user",
            &[],
        )
        .unwrap();
    let mut eligible = |usage: &str| -> Vec<String> {
        c.query(
            "SELECT rolname FROM pg_catalog.pg_roles WHERE pg_has_role(session_user,oid,$1) ORDER BY rolname",
            &[&usage],
        )
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect()
    };
    json!({
        "session_user": row.get::<_, String>(0),
        "role_oid": row.get::<_, i64>(1),
        "rolsuper": row.get::<_, bool>(2),
        "rolbypassrls": row.get::<_, bool>(3),
        "rolcreatedb": row.get::<_, bool>(4),
        "rolcreaterole": row.get::<_, bool>(5),
        "rolreplication": row.get::<_, bool>(6),
        "temp_eligible": row.get::<_, bool>(7),
        "member_of": eligible("MEMBER"),
        "set_to": eligible("SET"),
    })
}

/// Installed metadata for one named function: owner, definer flag, ACL and
/// effective search_path entry. pg_catalog-qualified observation only.
fn function_metadata(c: &mut Client, signature: &'static str) -> Value {
    let Some(row) = c
        .query_opt(
            "SELECT o.rolname,p.prosecdef,coalesce(p.proacl::text,''),coalesce(p.proconfig,'{}'::text[]) \
             FROM pg_catalog.pg_proc p JOIN pg_catalog.pg_roles o ON o.oid=p.proowner \
             WHERE p.oid::regprocedure::text=$1",
            &[&signature],
        )
        .unwrap()
    else {
        return json!({"signature": signature, "missing": true,
                      "safe_relation_resolution": false});
    };
    let config: Vec<String> = row.get(3);
    let search_path = config
        .iter()
        .find(|e| e.starts_with("search_path="))
        .map(|e| e["search_path=".len()..].to_string());
    json!({
        "signature": signature,
        "owner": row.get::<_, String>(0),
        "security_definer": row.get::<_, bool>(1),
        "acl": row.get::<_, String>(2),
        "search_path": search_path,
        "safe_relation_resolution": search_path
            .as_deref()
            .map(search_path_safe)
            .unwrap_or(false),
    })
}

/// Effective relation resolution is safe only when pg_temp is explicitly
/// listed after pg_catalog; an omitted pg_temp still resolves first.
fn search_path_safe(value: &str) -> bool {
    let parts: Vec<&str> = value.split(',').map(str::trim).collect();
    match (
        parts.iter().position(|s| *s == "pg_catalog"),
        parts.iter().position(|s| *s == "pg_temp"),
    ) {
        (Some(catalog), Some(temp)) => catalog < temp,
        _ => false,
    }
}

/// The 25 installed functions' metadata; runs as the admin fixture role.
pub(crate) fn altered_function_metadata() -> Vec<Value> {
    let mut a = admin();
    ALTERED_FUNCTIONS
        .iter()
        .map(|s| function_metadata(&mut a, s))
        .collect()
}

/// Bounded call observation: reached outcome and closed error text only.
pub(crate) fn try_call(c: &mut Client, sql: &str, params: &[&(dyn ToSql + Sync)]) -> Value {
    match c.query_one(sql, params) {
        Ok(row) => {
            // void results and NULLs both record value_present=false; typed
            // return values (jsonb) deserialize to Some.
            let value: Option<Value> = row.try_get(0).unwrap_or_default();
            json!({"ok": true, "value_present": value.is_some()})
        }
        Err(error) => json!({
            "ok": false,
            "error": db_message(&error),
            "code": error.code().map(|s| s.code().to_string()),
        }),
    }
}

/// TEMP pg_temp.pg_roles shadow mapping this session's own rolname to the
/// victim writer oid. It carries every column the definer functions read
/// (oid, rolname and the power flags private_live_actor checks) so the
/// boundary outcome distinguishes a real writer bypass from a lookup error.
pub(crate) fn shadow_writer_oid(c: &mut Client, writer_oid: postgres::types::Oid) {
    // The temp relation must be function-owner-readable: a definer's table
    // privilege is checked against the function owner, not the caller.
    c.batch_execute(
        "DROP TABLE IF EXISTS pg_temp.pg_roles; \
         CREATE TEMP TABLE pg_roles(oid oid,rolname name,rolsuper boolean,rolbypassrls boolean,\
         rolcreatedb boolean,rolcreaterole boolean,rolreplication boolean); \
         GRANT SELECT ON TABLE pg_temp.pg_roles TO PUBLIC",
    )
    .unwrap();
    c.execute(
        "INSERT INTO pg_temp.pg_roles \
         VALUES($1,session_user,false,false,false,false,false)",
        &[&writer_oid],
    )
    .unwrap();
}

/// Harmless empty catalog-shaped temporary relations for qualification ports.
pub(crate) fn empty_catalog_shadows(c: &mut Client) {
    for name in ["pg_roles", "pg_class", "pg_namespace", "pg_proc"] {
        c.batch_execute(&format!(
            "DROP TABLE IF EXISTS pg_temp.{name}; \
             CREATE TEMP TABLE {name} AS SELECT * FROM pg_catalog.{name} WHERE false"
        ))
        .unwrap();
    }
}

/// Remove every temporary catalog shadow created in this session.
pub(crate) fn drop_catalog_shadows(c: &mut Client) {
    c.batch_execute(
        "DROP TABLE IF EXISTS pg_temp.pg_roles; \
         DROP TABLE IF EXISTS pg_temp.pg_class; \
         DROP TABLE IF EXISTS pg_temp.pg_namespace; \
         DROP TABLE IF EXISTS pg_temp.pg_proc",
    )
    .unwrap();
}

/// Lease custodian for slow environments: autonomous_control.lease_until_ms
/// is only now+2000 and one run_renew call can run long on a loaded fixture,
/// so a single 400 ms custodian leaves periodic dead gaps. Two independent
/// restricted-Broker connections renew on an offset tight cadence so at least
/// one commit lands inside every lease window. Failures are silent: the pair
/// is redundant, and a real lease loss surfaces at the next gated call.
pub(crate) struct LeaseGuard {
    stop: Arc<AtomicBool>,
    workers: Vec<std::thread::JoinHandle<()>>,
}
impl LeaseGuard {
    pub(crate) fn start(run: Uuid) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let workers = (0..2)
            .map(|offset| {
                let stopping = stop.clone();
                std::thread::spawn(move || {
                    let mut runs = duskweave::postgres_m1_run::PgRunStore::new(broker());
                    if offset == 1 {
                        std::thread::sleep(Duration::from_millis(700));
                    }
                    while !stopping.load(Ordering::Relaxed) {
                        let _ = runs.renew(run);
                        std::thread::sleep(Duration::from_millis(120));
                    }
                })
            })
            .collect();
        Self { stop, workers }
    }
}
impl Drop for LeaseGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

/// Scoped run/control/events, reservation/call state/usage/binding and
/// result/charge/disposal facts for one run and scope, ordered for equality.
pub(crate) fn scoped_snapshot(run: Uuid, e: Uuid, c: Uuid) -> Value {
    let mut a = admin();
    let rows = |a: &mut Client, sql: &str, params: &[&(dyn ToSql + Sync)]| -> Value {
        a.query_one(sql, params).unwrap().get::<_, Value>(0)
    };
    json!({
        "runs": rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.autonomous_runs t WHERE t.run_id=$1", &[&run]),
        // lease_until_ms is ambient renewal timing, not a protected fact.
        "control": rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t)-'lease_until_ms' ORDER BY to_jsonb(t)-'lease_until_ms'),'[]'::jsonb) FROM execution.autonomous_control t WHERE t.run_id=$1", &[&run]),
        "sessions": rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.live_effect_sessions t WHERE t.run_id=$1", &[&run]),
        "effect_control": rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.live_effect_control t WHERE t.run_id=$1", &[&run]),
        "events": rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.live_effect_events t WHERE t.run_id=$1", &[&run]),
        "results": rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.live_dns_results t WHERE t.run_id=$1", &[&run]),
        "calls": rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM (SELECT call,state,usage,binding FROM execution.inference_calls t WHERE t.e=$1 AND t.c=$2) t", &[&e, &c]),
        "reservations": rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM (SELECT generation,record_revision,kind,writer_oid FROM execution.inference_reservations t WHERE t.engagement_id=$1 AND t.campaign_id=$2) t", &[&e, &c]),
        "fences": rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.session_fences t WHERE t.engagement_id=$1 AND t.campaign_id=$2", &[&e, &c]),
    })
}
