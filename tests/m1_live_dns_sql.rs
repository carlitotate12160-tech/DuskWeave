//! Direct restricted-SQL regression target for the schema-0030 live lane.
//! The ordinary suite runs only the wrapper: it provisions one dedicated
//! dw_native_ fixture through the existing bootstrap and re-enters this same
//! instrumented executable for the ignored `sql_live` assertion group. The
//! assertions themselves execute only inside that supervised worker against
//! real restricted roles; no privileged acquisition, model or target contact.
#![allow(dead_code)]

#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_genai_fixture.rs"]
mod genai;
#[path = "support/m1_terrain.rs"]
mod terrain_support;

use duskweave::m1_effect_control::authority::RestrictedWithdrawal;
use duskweave::mission::{CampaignId, EngagementId, OperationId, OperatorRef};
use duskweave::postgres_m1_effect::PgStopWriter;
use duskweave::registration;
use duskweave::withdrawal::{WithdrawalReason, WithdrawalRequest};
use postgres::{Client, NoTls};
use serde_json::{Value, json};
use uuid::Uuid;

const MARKER: &str = "DW_TEST_M1_LIVE_SQL_WORKER";
const GROUP: &str = "sql_live";
const RELEASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const LAB: &str = "autonomous-dns-genai-lab-v1";

fn worker() -> bool {
    std::env::var(MARKER).ok().as_deref() == Some("sql-worker-v1")
}

fn db_message(error: &postgres::Error) -> String {
    error
        .as_db_error()
        .map(|e| e.message().to_string())
        .unwrap_or_else(|| format!("{error:?}"))
}

fn unwrap_db<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|e| panic!("{e:?}"))
}

fn client(variable: &str) -> Client {
    std::env::var(variable)
        .expect("fixture DSN")
        .parse::<postgres::Config>()
        .expect("fixture DSN parse")
        .connect(NoTls)
        .expect("fixture connection")
}

fn admin() -> Client {
    let mut cfg = std::env::var("DW_TEST_ADMIN_DATABASE_URL")
        .expect("admin DSN")
        .parse::<postgres::Config>()
        .unwrap();
    cfg.dbname(&std::env::var("DW_NATIVE_FIXTURE_DATABASE").unwrap());
    cfg.connect(NoTls).unwrap()
}

fn stop_writer() -> Client {
    client("DW_STOP_WRITER_DATABASE_URL")
}

fn broker() -> Client {
    client("DW_INFERENCE_BROKER_DATABASE_URL")
}

fn register_live(slot: u128, live: bool) -> (Uuid, Uuid, Uuid) {
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
fn register_live_campaign(
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

fn withdrawal_q(e: Uuid, c: Uuid, operator: Uuid) -> Value {
    json!({"engagement_id":e,"campaign_id":c,"operator_ref":operator,
        "expected_mission_revision":1,"reason":"operator_requested"})
}

fn live_withdrawal(
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
fn fabricate_live_run(e: Uuid, c: Uuid, session: bool, running: bool) -> (Uuid, i64, Uuid) {
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

#[test]
fn dedicated_schema0030_fixture_executes_the_sql_assertion_group() {
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
            "0030",
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
        "dedicated schema-0030 fixture failed; report {}: {stdout}\n{stderr}",
        report.display()
    );
    let passed = stdout
        .split("test result: ok. ")
        .nth(1)
        .and_then(|rest| rest.split(" passed").next())
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap_or(0);
    assert!(passed >= 5, "SQL assertion group must execute: {stdout}");
}

// ------------------------- supervised worker assertions -------------------------

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
fn sql_live_v4_admission_requires_single_canonical_seed() {
    assert!(worker());
    let (e, c, _operator) = register_live(0x5c01, true);
    let now: i64 = admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    let asset = Uuid::new_v4();
    let request = |seeds: Value, version: &str, profile: &str| {
        let mut q = json!({"version":version,"run_id":Uuid::new_v4(),"run_profile":profile,
            "execution_mode":if version=="m1_autonomous_run_request_v4" {"live"} else {"controlled_fixture"},
            "deadline_at_ms":now+300000,"inference_profile":"fixture-genai-responses-v1",
            "context":{"version":"m1_context_prepare_request_v1","mission":{"engagement_id":e,"campaign_id":c},
                "candidates":seeds.as_array().unwrap().iter().map(|s| json!({"asset_ref":s["asset_ref"],"priority":"high"})).collect::<Vec<_>>()},
            "seed_names":seeds});
        if version == "m1_autonomous_run_request_v4" {
            q["live_binding"] = json!({"qualification_ref":Uuid::new_v4(),"release_sha256":RELEASE,"resolver_ipv4":"192.0.2.53"});
        } else {
            q.as_object_mut().unwrap().remove("live_binding");
        }
        q
    };
    let seed = |name: &str| json!({"asset_ref":asset,"name":name});
    let v4 = "m1_autonomous_run_request_v4";
    let v3 = "m1_autonomous_run_request_v3";
    let start = |q: &Value| broker().query_one("SELECT execution.run_start($1,$2)", &[q, &RELEASE]);
    // Exactly one v4 seed/candidate: two seeds refuse before any write.
    for q in [
        request(
            json!([seed("a.duskweave.test"), seed("b.duskweave.test")]),
            v4,
            LAB,
        ),
        request(json!([seed(".duskweave.test")]), v4, LAB),
        request(json!([seed("duskweave.test")]), v4, LAB),
        request(json!([seed("other.test")]), v4, LAB),
        request(json!([seed("seed.example.com")]), v4, LAB),
    ] {
        let error = start(&q).unwrap_err();
        assert!(
            db_message(&error).contains("run_request_refused"),
            "{q}: {error}"
        );
    }
    // A canonical LAB name with a nonempty (possibly multi-label) prefix is
    // consistent with Rust admission and passes the seed gate, refusing only
    // later at authority publication. Legacy versions keep 1-8 seed semantics.
    for q in [
        request(json!([seed("a.b.duskweave.test")]), v4, LAB),
        request(json!([seed("seed.duskweave.test")]), v4, LAB),
        request(
            json!([seed("x.example"), seed("y.example")]),
            v3,
            "autonomous-dns-fixture-v2",
        ),
    ] {
        let error = start(&q).unwrap_err();
        assert!(
            !db_message(&error).contains("run_request_refused"),
            "{q}: {error}"
        );
        assert!(
            db_message(&error).contains("run_authority_unavailable"),
            "{q}: {error}"
        );
    }
    // Refusals consumed no run identity (leases/sessions cannot exist without one).
    assert_eq!(
        admin()
            .query_one(
                "SELECT count(*) FROM execution.autonomous_runs WHERE e=$1",
                &[&e]
            )
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_receipt_binds_qualification_fields() {
    assert!(worker());
    let (e, c, _operator) = register_live(0x5d01, true);
    let (run, _, _) = fabricate_live_run(e, c, false, true);
    let qualified = |run: Uuid| -> bool {
        broker()
            .query_one(
                "SELECT execution.run_summary_without_diagnostics($1)",
                &[&run],
            )
            .unwrap()
            .get::<_, Value>(0)["live_qualified"]
            .as_bool()
            .unwrap()
    };
    assert!(
        qualified(run),
        "matching manifest qualification is live_qualified"
    );
    // Receipt comparison binds release, not merely the qualification ref.
    admin()
        .execute(
            "UPDATE execution.live_qualifications SET release_sha256=$1 WHERE run_profile=$2",
            &[&"f".repeat(64), &LAB],
        )
        .unwrap();
    assert!(!qualified(run), "release mismatch cannot be live_qualified");
    admin()
        .execute(
            "UPDATE execution.live_qualifications SET valid_until_ms=2 WHERE run_profile=$1",
            &[&LAB],
        )
        .unwrap();
    assert!(
        !qualified(run),
        "expired qualification cannot be live_qualified"
    );
}

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_terrain_integrity_hold_persists_under_active_fence() {
    assert!(worker());
    let _g = terrain_support::db();
    let (e, c, _ev, now) = register_live_campaign(0x5e01);
    // Accept one ordinary observation first (unfenced scope).
    let req = terrain_support::parse_req(&terrain_support::dns_request_json(e, c, now - 20));
    let mut terrain = terrain_support::terrain_client();
    let op = terrain_support::allocate_op();
    let event = duskweave::m1_terrain::evaluate_dns_observation(
        &req,
        None,
        duskweave::m1_terrain_input::TerrainIds::derive(e, c, op).unwrap(),
        (_ev.0, _ev.1, now),
    )
    .unwrap();
    let mut tx = terrain.build_transaction().isolation_level(postgres::IsolationLevel::Serializable).start().unwrap();
    terrain_support::sql_accept(
        &mut tx,
        &serde_json::to_value(&req).unwrap(),
        &serde_json::to_value(&event).unwrap(),
    )
    .unwrap();
    tx.commit().unwrap();
    // Charge the live fence over the same scope with a committed result lineage.
    let (run, generation, result_ref) = fabricate_live_run(e.0, c.0, true, true);
    admin()
        .execute(
            "UPDATE execution.autonomous_runs SET terrain_operation=$1 WHERE run_id=$2",
            &[&op.0, &run],
        )
        .unwrap();
    admin()
        .execute(
            "INSERT INTO execution.live_dns_results SELECT $1,$2,x.attempt,$3,$4,2,'terrain.issued_dns.v2' FROM execution.autonomous_runs x WHERE x.run_id=$2",
            &[&result_ref, &run, &generation, &json!({"status":"completed"})],
        )
        .unwrap();
    // The qualified Terrain owner's validated conflict command is admitted and
    // the hold persists before any refusal; the current view observes it.
    let mut variant = req.clone();
    variant.perceived_at += 5;
    let mut raw = serde_json::to_value(&variant).unwrap();
    raw["answers"] = json!([{"ipv4":"198.51.100.99","ttl_seconds":60}]);
    let mut terrain = terrain_support::terrain_client();
    let mut tx = terrain
        .build_transaction()
        .isolation_level(postgres::IsolationLevel::Serializable)
        .start()
        .unwrap();
    let blocks: bool = tx
        .query_one(
            "SELECT terrain.record_dns_integrity_conflict($1,$2)",
            &[&raw, &op.0],
        )
        .unwrap()
        .get(0);
    tx.commit().unwrap();
    assert!(blocks);
    let holds = admin()
        .query_one(
            "SELECT count(*) FROM terrain.integrity_conflicts WHERE operation_id=$1 AND blocks_use",
            &[&op.0],
        )
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(
        holds, 1,
        "restrictive hold persisted under the active fence"
    );
    // A committed Mission withdrawal in between does not release the persisted
    // hold, and the withdrawn mission can no longer accept terrain commands.
    live_withdrawal(
        &withdrawal_q(e.0, c.0, Uuid::from_u128(0x11)),
        Uuid::new_v4(),
        None,
        false,
    )
    .unwrap();
    assert_eq!(
        admin()
            .query_one(
                "SELECT count(*) FROM terrain.integrity_conflicts WHERE operation_id=$1 AND blocks_use",
                &[&op.0]
            )
            .unwrap()
            .get::<_, i64>(0),
        1,
        "withdrawal does not erase a committed integrity hold"
    );
    let mut replay_client = terrain_support::terrain_client();
    let mut tx = replay_client
        .build_transaction()
        .isolation_level(postgres::IsolationLevel::Serializable)
        .start()
        .unwrap();
    let replay = tx.query_one(
        "SELECT terrain.record_dns_integrity_conflict($1,$2)",
        &[&raw, &op.0],
    );
    assert!(
        replay.is_err(),
        "a withdrawn mission refuses further terrain commands: {replay:?}"
    );
    // Arbitrary writes remain denied: no ordinary SQL grant, no UPDATE/DELETE.
    assert!(
        terrain_support::terrain_client()
            .execute(
                "INSERT INTO terrain.integrity_conflicts VALUES($1,$2,$3,$4,1,true,'input_mismatch')",
                &[&e.0, &c.0, &Uuid::new_v4(), &Uuid::new_v4()]
            )
            .is_err(),
        "ordinary restricted INSERT stays denied"
    );
    assert!(
        admin()
            .execute(
                "DELETE FROM terrain.integrity_conflicts WHERE operation_id=$1",
                &[&op.0]
            )
            .is_err(),
        "the charged fence denies truth mutation even to setup roles"
    );
    // Runtime roles never reach the restrictive command.
    assert!(
        db_support::runtime_client()
            .query_one(
                "SELECT terrain.record_dns_integrity_conflict($1,$2)",
                &[&raw, &op.0]
            )
            .is_err()
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
