//! Target-private registration, request and connection setup for schema-0030 SQL assertions.
use duskweave::mission::{CampaignId, EngagementId, OperationId};
use duskweave::registration;
use postgres::{Client, NoTls};
use serde_json::{Value, json};
use uuid::Uuid;

pub(crate) use super::{consent, db_support, genai, terrain_support};

pub const MARKER: &str = "DW_TEST_M1_LIVE_SQL_WORKER";
pub const GROUP: &str = "sql_live";
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

pub fn unwrap_db<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
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
    register_shaped(slot, |raw| {
        let p = &mut raw["m1_inference_permission"];
        if live {
            p["version"] = json!("m1_inference_permission_v6");
            p["execution_mode"] = json!("live");
            p["run_profile"] = json!(LAB);
            p["live_binding"] = json!({"qualification_ref":Uuid::new_v4(),
                "release_sha256":RELEASE,"resolver_ipv4":"192.0.2.53"});
        }
    })
}

/// Narrow version/profile/binding shaping hook for successor cases; the
/// accepted registration path and timing fields remain unchanged.
pub fn register_shaped(slot: u128, shape: impl FnOnce(&mut Value)) -> (Uuid, Uuid, Uuid) {
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
    p["starts_at"] = json!(now);
    p["ends_at"] = json!(now + 3599);
    p["permission_id"] = json!(Uuid::new_v4());
    shape(&mut raw);
    let input =
        duskweave::input::parse_register(&serde_json::to_vec(&raw).unwrap()).expect("registration");
    let (mut alloc, mut store, mut traj) = db_support::ports();
    let op = registration::prepare_operation(&mut alloc).unwrap();
    registration::register(&mut alloc, &mut store, &mut traj, op, &input).expect("registered");
    (e.0, c.0, Uuid::from_u128(0x11))
}

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
        (
            OperationId(row.get(0)),
            duskweave::mission::EventId(row.get(1)),
        ),
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

pub fn fabricate_live_run(e: Uuid, c: Uuid, session: bool, running: bool) -> (Uuid, i64, Uuid) {
    let mut a = admin();
    let qual = Uuid::new_v4();
    let hex = |ch: char| ch.to_string().repeat(64);
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

pub fn now_ms() -> i64 {
    admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .unwrap()
        .get(0)
}

/// Read the accepted registration publication through the restricted broker port.
pub fn live_publication(e: Uuid, c: Uuid, run: Uuid) -> Value {
    broker()
        .query_one(
            "SELECT mission.current_inference_publication($1,$2,$3)",
            &[&e, &c, &run],
        )
        .unwrap()
        .get(0)
}

/// One accepted-binding qualification seeder for LAB and LIVE cases: inserts
/// a single synthetic fixture-bound row whose run_profile/purpose/resolver
/// derive from the accepted permission through the canonical profile mapping.
/// Digests/boot bytes are synthetic fixture values, never host evidence.
pub fn seed_live_qualification(publication: &Value) -> Uuid {
    let permission = &publication["fields"]["m1_inference_permission"];
    let binding = &permission["live_binding"];
    let run_profile = permission["run_profile"].as_str().unwrap();
    let purpose = duskweave::m1_inference_profile::sdk_versions(run_profile)
        .expect("canonical run profile")
        .qualification_purpose
        .expect("live-bound run profile carries a qualification purpose");
    let resolver = binding["resolver_ipv4"].as_str().unwrap();
    assert!(duskweave::m1_inference_profile::resolver_binding(
        run_profile,
        resolver
    ));
    assert_eq!(binding["release_sha256"], RELEASE);
    let qualification: Uuid = binding["qualification_ref"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let hex = |ch: char| ch.to_string().repeat(64);
    admin()
        .execute(
            "INSERT INTO execution.live_qualifications(qualification_ref,run_profile,purpose,release_sha256,launcher_sha256,capture_sha256,exporter_sha256,image_digest,manifest_sha256,boot_id,resolver_ipv4,valid_from_ms,valid_until_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
            &[
                &qualification,
                &run_profile,
                &purpose,
                &RELEASE,
                &hex('c'),
                &hex('d'),
                &hex('e'),
                &format!("sha256:{}", hex('f')),
                &hex('0'),
                &Uuid::new_v4(),
                &resolver.parse::<std::net::IpAddr>().unwrap(),
                &(now_ms() - 60000),
                &(now_ms() + 3600000),
            ],
        )
        .unwrap();
    qualification
}

pub struct PreparedDecision {
    pub scope: duskweave::m1_reasoning::inference::Scope,
    pub pack: duskweave::m1_context::PreparedContext,
    pub input_hash: String,
    pub window_ms: i64,
}

/// Real D1 preparation: restricted broker renew/start/claim, input binding and
/// ContextReader provenance. No fabricated pack or accounting rows.
pub fn prepare_decision_one(
    runs: &mut duskweave::postgres_m1_run::PgRunStore,
    record: &duskweave::m1_run::RunRecord,
) -> PreparedDecision {
    use duskweave::m1_run::RunPhase;
    use sha2::{Digest, Sha256};
    let run = record.request.run_id;
    runs.renew(run).unwrap();
    runs.start_phase(run, RunPhase::DecisionOne).unwrap();
    runs.claim_phase(run, RunPhase::DecisionOne, RELEASE)
        .unwrap();
    let q = &record.request.context;
    let e = q.mission.engagement_id.0;
    let c = q.mission.campaign_id.0;
    let decision = record.decision_one;
    let input_hash = Sha256::digest(serde_json::to_vec(q).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let mut port = broker();
    port.execute(
        "SELECT execution.bind_inference_input($1,$2,$3,$4)",
        &[&e, &c, &decision, &input_hash],
    )
    .unwrap();
    let window_ms: i64 = port
        .query_one(
            "SELECT execution.inference_window_ms($1,$2,$3)",
            &[&e, &c, &decision],
        )
        .unwrap()
        .get(0);
    let context = duskweave::m1_autonomous_context::AutonomousContext {
        binding: duskweave::m1_autonomous_context::RunContextBinding {
            engagement_id: q.mission.engagement_id,
            campaign_id: q.mission.campaign_id,
            run_profile: record.request.run_profile.clone(),
            execution_mode: record.request.execution_mode.clone(),
            inference_profile: record.request.inference_profile.clone(),
            release_sha256: record.release_sha256.clone(),
            qualification_ref: record.qualification_ref,
        },
        predecessor: None,
    };
    let pack = duskweave::m1_context::prepare_autonomous(
        &mut duskweave::postgres_mission::PgContextReader::new(broker()),
        q,
        duskweave::mission::OperationId(decision),
        &context,
    )
    .unwrap();
    PreparedDecision {
        scope: duskweave::m1_reasoning::inference::Scope {
            engagement: e,
            campaign: c,
            decision,
            mission_revision: 1,
            permission: q.mission.permission_id,
        },
        pack,
        input_hash,
        window_ms,
    }
}

pub fn run_fixture_wrapper() {
    if worker() {
        return;
    }
    let unique = format!(
        "{:x}_{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let database = format!("dw_native_livesql_{unique}");
    let database = &database[..database.len().min(63)];
    let report = std::env::temp_dir().join(format!("dw-livesql-{unique}"));
    let python = std::env::var_os("DW_TEST_PYTHON").unwrap_or_else(|| {
        if cfg!(windows) {
            "python".into()
        } else {
            "python3".into()
        }
    });
    let mut command = std::process::Command::new(python);
    command
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env(MARKER, "sql-worker-v1")
        .env_remove("DEEPSEEK_API_KEY")
        .arg("tests/support/m1_native_bootstrap.py")
        .args([
            "--container",
            if cfg!(windows) { "dwpg-r3" } else { "dwpg" },
        ]);
    if !cfg!(windows) {
        command.arg("--ci");
    }
    command
        .args([
            "--database",
            "fixture",
            "--database-name",
            database,
            "--schema-revision",
            "0033",
            "--role-prefix",
            &format!("dw_lsq{}", &unique[..unique.len().min(10)]),
            "--report",
        ])
        .arg(&report)
        .arg("--")
        .arg(std::env::current_exe().unwrap())
        .args([GROUP, "--ignored", "--test-threads=1", "--nocapture"]);
    let output = command.output().expect("bootstrap supervisor spawn");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "dedicated schema-0033 fixture failed; report {}: {stdout}\n{stderr}",
        report.display()
    );
    let log = std::fs::read_to_string(report.join("qualification.log"))
        .expect("complete sanitized worker log");
    let passed = log
        .split("test result: ok. ")
        .nth(1)
        .and_then(|rest| rest.split(" passed").next())
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap_or(0);
    for name in [
        "pro_profile::sql_live_pro_intent_generation_and_repair_are_profile_bound",
        "pro_profile::sql_live_pro_accounting_and_terminal_diagnostics_preserve_model_identity",
        "pro_profile::sql_live_pro_missing_key_refuses_and_key_free_recovery_preserves_identity",
        "successor_pack::sql_live_successor_pack_v2_v3_v4_contract_matrix",
        "diagnostic_episode::sql_live_proposal_diagnostics_survive_terminal_repair_and_recovery",
        "successor_episode::sql_live_sdk_v3_cli_feedback_and_key_free_recovery_on_schema0030",
        "audit007::sql_live_aud007_original_writer_isolation_survives_temp_pg_roles",
        "audit007::sql_live_aud007_inference_and_live_stop_refuse_foreign_writer",
        "audit007::sql_live_aud007_catalog_paths_and_qualification_use_real_catalogs",
    ] {
        let line = format!("test {name} ... ok");
        assert_eq!(log.matches(&line).count(), 1, "{name} missing: {log}");
    }
    assert!(passed >= 17, "SQL assertion group must execute: {log}");
    eprintln!(
        "schema0033_sql_group passed={passed} new=3 report={}",
        report.display()
    );
}
