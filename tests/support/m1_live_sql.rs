//! Restricted-SQL fixture helpers shared by the live DNS regression target.
//! All connections stay inside the supervised dw_native_ fixture; the
//! administrator handle fabricates only pre-requisite run/session rows.
#![allow(dead_code)]

#[path = "registration_db.rs"]
pub mod db_support;
#[path = "m1_genai_fixture.rs"]
mod genai;
#[path = "m1_inference_permission.rs"]
mod consent;
#[path = "m1_terrain.rs"]
pub mod terrain_support;

use duskweave::mission::{CampaignId, EngagementId, OperationId};
use duskweave::registration;
use postgres::{Client, NoTls};
use serde_json::{Value, json};
use uuid::Uuid;

pub const MARKER: &str = "DW_TEST_M1_LIVE_SQL_WORKER";
pub const RELEASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
pub const LAB: &str = "autonomous-dns-genai-lab-v1";
pub fn worker() -> bool {
    std::env::var(MARKER).ok().as_deref() == Some("sql-worker-v1")
}

pub fn db_message(error: &postgres::Error) -> String {
    error
        .as_db_error()
        .map(|e| e.message().to_string())
        .unwrap_or_else(|| format!("{error:?}"))
}

fn unwrap_db<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|e| panic!("{e:?}"))
}

pub fn client(variable: &str) -> Client {
    std::env::var(variable)
        .expect("fixture DSN")
        .parse::<postgres::Config>()
        .expect("fixture DSN parse")
        .connect(NoTls)
        .expect("fixture connection")
}

pub fn admin() -> Client {
    let mut cfg = std::env::var("DW_TEST_ADMIN_DATABASE_URL")
        .expect("admin DSN")
        .parse::<postgres::Config>()
        .unwrap();
    cfg.dbname(&std::env::var("DW_NATIVE_FIXTURE_DATABASE").unwrap());
    cfg.connect(NoTls).unwrap()
}

pub fn stop_writer() -> Client {
    client("DW_STOP_WRITER_DATABASE_URL")
}

pub fn broker() -> Client {
    client("DW_INFERENCE_BROKER_DATABASE_URL")
}

pub fn register_live(slot: u128, live: bool) -> (Uuid, Uuid, Uuid) {
    let (e, c) = db_support::scope(slot);
    let now: i64 = admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    let mut raw = consent::registration(e, c);
    genai::configure_permission(&mut raw);
    raw["starts_at"] = json!(now - 1);
    raw["ends_at"] = json!(now + 3600);
    raw["m1_permission"] = json!({
        "policy_version":1,"ct_base_domain":"duskweave.test","provider_disclosure":"crt_sh",
        "vantage_ref":Uuid::new_v4(),"resolver_ipv4":"192.0.2.53",
        "discovery_rules":[{"label_suffix":"duskweave.test"}],
        "contact_rules":[{"asset_ref":raw["included_assets"][0],"rule":{"exact":"seed.duskweave.test"},"priority":1}],
        "excluded_names":[],"approved_path":"/","starts_at":now,"ends_at":now+3599,
        "campaign_limits":{"episodes":1,"provider_calls":1,"dns_questions":1,"dns_followups":1,
            "tcp_connections":1,"head_requests":1},"concurrency":1});
    let p = &mut raw["m1_inference_permission"];
    if live {
        p["version"] = json!("m1_inference_permission_v6");
        p["execution_mode"] = json!("live");
        p["run_profile"] = json!(LAB);
        p["live_binding"] = json!({"qualification_ref":Uuid::new_v4(),
            "release_sha256":RELEASE,"resolver_ipv4":"192.0.2.53"});
    }
    p["starts_at"] = json!(now);
    p["ends_at"] = json!(now + 3599);
    p["permission_id"] = json!(Uuid::new_v4());
    let input =
        duskweave::input::parse_register(&serde_json::to_vec(&raw).unwrap()).expect("registration");
    let (mut alloc, mut store, mut traj) = db_support::ports();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    registration::register(&mut alloc, &mut store, &mut traj, op, &input).expect("registered");
    (e.0, c.0, Uuid::from_u128(0x11))
}

/// The terrain admission flow needs its ordinary m1_permission shape; the
/// withdrawal authority additionally requires a live-bound registration, so
/// this registers through the same helper with the live binding patched in.
pub fn register_live_campaign(
    slot: u128,
) -> (
    EngagementId,
    CampaignId,
    (OperationId, duskweave::mission::EventId),
    i64,
) {
    let (e, c) = db_support::scope(slot);
    let now = terrain_support::policy_support::now();
    let mut reg_val = terrain_support::policy_support::registration(e, c, now);
    // Full permission skeleton (v1 shape) with this registration's own basis.
    let mut wrap = json!({"m1_inference_permission":
        consent::registration(e, c)["m1_inference_permission"]});
    genai::configure_permission(&mut wrap);
    let p = &mut wrap["m1_inference_permission"];
    for key in [
        "engagement_id",
        "campaign_id",
        "operator_ref",
        "authority_ref",
        "authority_revision",
        "goal_ref",
        "exercise_mode",
    ] {
        p[key] = reg_val[key].clone();
    }
    p["version"] = json!("m1_inference_permission_v6");
    p["execution_mode"] = json!("live");
    p["run_profile"] = json!(LAB);
    p["live_binding"] = json!({"qualification_ref":Uuid::new_v4(),
        "release_sha256":RELEASE,"resolver_ipv4":"192.0.2.53"});
    p["starts_at"] = json!(now);
    p["ends_at"] = json!(now + 3599);
    p["permission_id"] = json!(Uuid::new_v4());
    reg_val["m1_inference_permission"] = p.clone();
    let input = duskweave::input::parse_register(&serde_json::to_vec(&reg_val).unwrap()).unwrap();
    let (mut alloc, mut store, mut traj) = db_support::ports();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    registration::register(&mut alloc, &mut store, &mut traj, op, &input).unwrap();
    let row = admin()
        .query_one(
            "SELECT operation_id,event_id FROM mission.registration_outbox WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap();
    (
        e,
        c,
        (OperationId(row.get(0)), duskweave::mission::EventId(row.get(1))),
        now,
    )
}

pub fn withdrawal_q(e: Uuid, c: Uuid, operator: Uuid) -> Value {
    json!({"engagement_id":e,"campaign_id":c,"operator_ref":operator,
        "expected_mission_revision":1,"reason":"operator_requested"})
}

pub fn live_withdrawal(
    q: &Value,
    op: Uuid,
    generation: Option<i64>,
    recover: bool,
) -> Result<Value, postgres::Error> {
    let row = stop_writer().query_one(
        "SELECT mission.live_stop_withdrawal($1,$2,$3,$4)",
        &[&q, &op, &generation, &recover],
    )?;
    row.try_get(0)
}

/// Administrator fixture setup only: one v4 run under an optional charged
/// session so the fence is active. Returns (run, generation, result_ref).
pub fn fabricate_live_run(e: Uuid, c: Uuid, session: bool, running: bool) -> (Uuid, i64, Uuid) {
    let mut a = admin();
    let qual = Uuid::new_v4();
    let hex = |c: char| c.to_string().repeat(64);
    a.execute(
        "INSERT INTO execution.live_qualifications VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        &[
            &qual,
            &LAB,
            &"LAB",
            &RELEASE,
            &hex('c'),
            &hex('d'),
            &hex('e'),
            &format!("sha256:{}", hex('f')),
            &hex('0'),
            &Uuid::new_v4(),
            &"192.0.2.53".parse::<std::net::IpAddr>().unwrap(),
            &(1_i64),
            &(i64::MAX / 2),
        ],
    )
    .unwrap();
    let run = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    let result = Uuid::new_v4();
    let (d1, d2, terrain_op) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let request = json!({"version":"m1_autonomous_run_request_v4","run_id":run,"run_profile":LAB,
        "execution_mode":"live","deadline_at_ms":i64::MAX/2,"inference_profile":"fixture-genai-responses-v1",
        "live_binding":{"qualification_ref":qual,"release_sha256":RELEASE,"resolver_ipv4":"192.0.2.53"},
        "context":{"version":"m1_context_prepare_request_v1","mission":{},"candidates":[]},
        "seed_names":[{"asset_ref":Uuid::new_v4(),"name":"seed.duskweave.test"}]});
    let writer: postgres::types::Oid = broker()
        .query_one("SELECT oid FROM pg_roles WHERE rolname=session_user", &[])
        .unwrap()
        .get(0);
    a.execute(
        "INSERT INTO execution.autonomous_runs VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        &[
            &run,
            &e,
            &c,
            &writer,
            &request,
            &d1,
            &d2,
            &attempt,
            &result,
            &terrain_op,
            &RELEASE,
            &qual,
            &(i64::MAX / 2),
        ],
    )
    .unwrap();
    a.execute(
        "INSERT INTO execution.autonomous_control VALUES($1,$2,'running','acquisition',true)",
        &[&run, &(i64::MAX / 2)],
    )
    .unwrap();
    let generation = 7_i64;
    if session {
        a.execute(
            "INSERT INTO execution.live_effect_sessions VALUES($1,$2,$3,$4,$5,1,$6,$7,$8,$9,$10,$11)",
            &[&run, &e, &c, &generation, &writer, &d1, &attempt, &result, &request, &qual, &(i64::MAX / 2)],
        )
        .unwrap();
        a.execute(
            "INSERT INTO execution.live_effect_control VALUES($1,'reserved',false,$2,false)",
            &[&run, &(i64::MAX / 2)],
        )
        .unwrap();
        a.execute(
            "INSERT INTO execution.live_effect_events VALUES($1,$2,'reserved',1)",
            &[&run, &generation],
        )
        .unwrap();
        if !running {
            a.execute(
                "UPDATE execution.autonomous_control SET status='stopped',lease_until_ms=0 WHERE run_id=$1",
                &[&run],
            )
            .unwrap();
        }
    }
    (run, generation, result)
}

