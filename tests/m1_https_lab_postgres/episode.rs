use crate::admission::{broker, lab_run_request, now_ms, register_lab_campaign, seed_lab_qual};
use crate::genai::{Reply, Server, envelope};
use crate::lab_support;
use crate::recovery::{FakeDnsBoundary, Renewals};
use crate::terrain::terrain_support::native;
use duskweave::m1_effect_control::acquire as acquire_dns;
use duskweave::m1_effect_control::https::{HttpsBoundary, HttpsEvent, acquire_with_clock};
use duskweave::m1_https::{HttpsIpcRequest, HttpsOutcome, HttpsPhase};
use duskweave::m1_inference_profile::HTTPS_LAB_RUN_PROFILE;
use duskweave::m1_run::RunPhase;
use duskweave::m1_terrain_issued::IssuedDnsReference;
use duskweave::mission::{CampaignId, EngagementId, OperationId};
use duskweave::postgres_m1_effect::https_loop::{PgAttemptDnsStore, PgHttpsAttemptStore};
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::postgres_m1_terrain::{PgTerrainStore, https::PgHttpsTerrainStore};
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_terrain_history::{record_https_terrain_history, record_terrain_history};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use uuid::Uuid;

struct ControlDir(PathBuf);
impl ControlDir {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("dw-hlab-ctrl-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        #[cfg(windows)]
        {
            let system = std::env::var_os("SystemRoot").unwrap();
            let script = "$ErrorActionPreference='Stop'; \
                $s=[Security.Principal.WindowsIdentity]::GetCurrent().User; \
                $a=New-Object Security.AccessControl.DirectorySecurity; $a.SetOwner($s); \
                $a.SetAccessRuleProtection($true,$false); \
                $r=New-Object Security.AccessControl.FileSystemAccessRule($s,'FullControl','ContainerInherit,ObjectInherit','None','Allow'); \
                $a.AddAccessRule($r); Set-Acl -LiteralPath $env:DW_M1_RUN_CONTROL_ROOT -AclObject $a";
            let ok = Command::new(
                PathBuf::from(&system).join("System32/WindowsPowerShell/v1.0/powershell.exe"),
            )
            .env_clear()
            .env("SystemRoot", system)
            .env("DW_M1_RUN_CONTROL_ROOT", &root)
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .status()
            .unwrap()
            .success();
            assert!(ok);
        }
        Self(root)
    }
}
impl Drop for ControlDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pack_in(value: &Value) -> Option<Value> {
    match value {
        Value::String(s) => serde_json::from_str::<Value>(s)
            .ok()
            .filter(|p| p["version"] == duskweave::m1_https::VERSION_HTTPS_LOOP),
        Value::Array(a) => a.iter().find_map(pack_in),
        Value::Object(o) => o.values().find_map(pack_in),
        _ => None,
    }
}

struct FakeHttpsBoundary {
    req: Option<HttpsIpcRequest>,
    step: u8,
}
impl HttpsBoundary for FakeHttpsBoundary {
    fn ready_connect(&mut self, req: &HttpsIpcRequest) -> duskweave::Res<()> {
        self.req = Some(req.clone());
        Ok(())
    }
    fn start_phase(&mut self, req: &HttpsIpcRequest, _: HttpsPhase, _: i64) -> duskweave::Res<()> {
        self.req = Some(req.clone());
        Ok(())
    }
    fn renew(&mut self, _: &HttpsIpcRequest, _: i64) -> duskweave::Res<()> {
        Ok(())
    }
    fn poll(&mut self) -> duskweave::Res<Option<HttpsEvent>> {
        self.step += 1;
        let req = self.req.as_ref().expect("start_phase must set req");
        match self.step {
            1 => Ok(Some(HttpsEvent::Ready(HttpsPhase::Tls))),
            2 => Ok(Some(HttpsEvent::Ready(HttpsPhase::Head))),
            3 => Ok(Some(HttpsEvent::Result(
                lab_support::sample_https_safe_result(
                    req,
                    HttpsOutcome::HeadResponse,
                    req.deadline_at_ms - i64::from(req.max_duration_ms),
                    200,
                    "none",
                    None,
                ),
            ))),
            _ => Ok(None),
        }
    }
    fn acknowledge(&mut self, _: &HttpsIpcRequest) -> duskweave::Res<bool> {
        Ok(true)
    }
    fn stop(&mut self) -> duskweave::Res<bool> {
        Ok(true)
    }
}

fn cli_cmd() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    c.env_clear()
        .env("DW_DATABASE_CONFIG_MODE", "env-local")
        .env(
            "DW_DATABASE_URL",
            std::env::var("DW_TEST_DATABASE_URL").unwrap(),
        );
    for key in [
        "DW_INFERENCE_BROKER_DATABASE_URL",
        "DW_COGNITION_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL",
        "DW_TERRAIN_DATABASE_URL",
        "SystemRoot",
        "LLVM_PROFILE_FILE",
    ] {
        if let Some(v) = std::env::var_os(key) {
            c.env(key, v);
        }
    }
    c
}

fn cli_json(args: &[&str]) -> (bool, Value) {
    let out = cli_cmd().args(args).output().unwrap();
    let txt = String::from_utf8_lossy(&out.stdout);
    let parsed = txt
        .lines()
        .rev()
        .find(|l| l.starts_with('{'))
        .map(|l| serde_json::from_str(l).unwrap())
        .unwrap_or_else(
            || json!({"raw": txt.trim(), "err": String::from_utf8_lossy(&out.stderr).trim()}),
        );
    (out.status.success(), parsed)
}

fn run_decision_phase(runs: &mut PgRunStore, run_id: Uuid, phase: RunPhase, ctrl: &ControlDir) {
    runs.start_phase(run_id, phase).unwrap();
    let mut child = cli_cmd()
        .env("DW_M1_RUN_CONTROL_ROOT", &ctrl.0)
        .env("DW_M1_RUN_PROVIDER_DECISION", "1")
        .args([
            "m1-run-phase",
            "--run",
            &run_id.to_string(),
            "--phase",
            phase.name(),
            "--timing-profile",
            "reasoning-bounded-pro-v2",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _stdin_keepalive = child.stdin.take().unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(st) = child.try_wait().unwrap() {
            break st;
        }
        assert!(Instant::now() < deadline, "m1-run-phase timeout");
        std::thread::sleep(Duration::from_millis(10));
    };
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        status.success(),
        "m1-run-phase {} failed: stdout={stdout}; stderr={}",
        phase.name(),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains(&format!("m1_run_ready_v1 {run_id} {}", phase.name())));
    runs.complete_phase(run_id, phase).unwrap();
}

#[ignore]
#[test]
fn cli_m1_run_live_mode_refuses_in_preflight_without_db_or_provider() {
    let _guard = native::qualified_existing_db();
    let server = Server::lab(|_| Reply::Length(200, vec![]));
    let qref = Uuid::new_v4();
    let rel = lab_support::sample_release_sha();
    seed_lab_qual(qref);
    let e = Uuid::new_v4();
    let c = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let reg = crate::admission::base_registration_json(e, c, qref, &rel);
    register_lab_campaign(&reg).unwrap();
    let req = lab_run_request(&reg, run_id, qref, &rel);
    let input = std::env::temp_dir().join(format!("dw-hlab-preflight-{run_id}.json"));
    std::fs::write(&input, serde_json::to_vec(&req).unwrap()).unwrap();
    let ctrl = ControlDir::new();
    let (ok, receipt) = cli_json(&[
        "m1-run",
        "--input",
        input.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-pro-v2",
    ]);
    let _ = (ctrl, std::fs::remove_file(&input));
    assert!(
        !ok,
        "live m1-run must refuse without qualified Linux file-mode host"
    );
    assert_eq!(server.count(), 0);
    let txt = receipt.to_string();
    assert!(
        txt.contains("live_host_platform_unqualified")
            || txt.contains("live_database_file_mode_required"),
        "unexpected preflight refusal: {txt}"
    );
    let count: i64 = native::admin()
        .query_one(
            "SELECT count(*) FROM execution.autonomous_runs WHERE run_id=$1",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
}

fn cli_release_sha() -> String {
    let bytes = std::fs::read(env!("CARGO_BIN_EXE_duskweave")).unwrap();
    lab_support::sha256_hex(&bytes)
}

pub(crate) fn start_cli_lab_run(
    qref: Uuid,
) -> (
    Uuid,
    Uuid,
    Uuid,
    Value,
    PgRunStore,
    duskweave::m1_run::RunRecord,
    String,
) {
    let release = cli_release_sha();
    let now = crate::admission::now_ms();
    crate::admission::seed_qual(
        qref,
        HTTPS_LAB_RUN_PROFILE,
        "HTTPS_LOOP_LAB",
        duskweave::m1_inference_profile::HTTPS_LAB_RESOLVER,
        &release,
        now - 60_000,
        now + 3_600_000,
    );
    let e = Uuid::new_v4();
    let c = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    let reg = crate::admission::base_registration_json(e, c, qref, &release);
    register_lab_campaign(&reg).unwrap();
    let req_val = lab_run_request(&reg, run_id, qref, &release);
    let req = duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&req_val).unwrap()).unwrap();
    let mut runs = PgRunStore::new(broker());
    assert!(runs.start(&req, &release).unwrap());
    let rec = runs.inspect(run_id).unwrap();
    (e, c, run_id, reg, runs, rec, release)
}

fn step_prop(refs: &[&str], next: Value) -> Value {
    json!({
        "version": duskweave::m1_https::VERSION_HTTPS_LOOP, "hypothesis": "Synthetic LAB step.",
        "considered_alternative": "alt", "references": refs, "expected_observation": "obs",
        "falsifying_observation": "fail", "uncertainty": "Synthetic LAB check.",
        "rationale": "Bounded LAB check.", "next_intent": next
    })
}

fn defer_intent() -> Value {
    json!({"category":"defer","references":["mission"],"manifest_reference":null,"dependency_refresh":"none"})
}

#[ignore]
#[test]
fn cli_m1_run_phase_d1_defer_and_inspect_recover_on_schema0038() {
    let _guard = native::qualified_existing_db();
    let server = Server::lab(|req| {
        let wire: Value = serde_json::from_slice(&req.body).unwrap();
        assert!(pack_in(&wire).is_some());
        let prop = step_prop(&["mission"], defer_intent());
        Reply::Length(200, serde_json::to_vec(&envelope(&prop)).unwrap())
    });
    let qref = Uuid::new_v4();
    let (_, _, run_id, _, mut runs, _, _) = start_cli_lab_run(qref);
    let ctrl = ControlDir::new();
    let renewals = Renewals::start(run_id);
    run_decision_phase(&mut runs, run_id, RunPhase::DecisionOne, &ctrl);
    runs.terminate_https_loop(run_id, 1, "deferred").unwrap();
    renewals.stop();
    runs.stop(run_id).unwrap();
    assert_eq!(server.count(), 1);

    for cmd in ["m1-run-inspect", "m1-run-recover"] {
        let (ok, v) = cli_json(&[cmd, "--run", &run_id.to_string()]);
        assert!(ok, "{cmd}: {v}");
        assert_eq!(v["status"], json!("deferred"));
        assert_eq!(v["live_qualified"], json!(true));
        assert_eq!(v["decision_count"], json!(1));
        assert_eq!(v["dns_effect_count"], json!(0));
        assert_eq!(v["head_effect_count"], json!(0));
    }
}

#[ignore]
#[test]
fn cli_m1_run_phase_three_decision_live_episode_and_recover_parity() {
    let _guard = native::qualified_existing_db();
    let server = Server::lab(|req| {
        let wire: Value = serde_json::from_slice(&req.body).unwrap();
        let pack: Value = pack_in(&wire).expect("https_loop context pack");
        let refs = pack["references"].as_array().unwrap();
        let https_count = refs
            .iter()
            .filter(|r| r["id"].as_str().is_some_and(|id| id.starts_with("https_")))
            .count();
        let feedback = pack["predecessor"]["availability"] == "committed";
        let prop = if !feedback && https_count == 0 {
            assert!(refs.iter().any(|r| r["id"] == "asset_0"));
            step_prop(
                &["mission", "asset_0"],
                json!({"category":"observe_more","references":["mission","asset_0"],"manifest_reference":"dns_udp","view_request":null,"dependency_refresh":"none"}),
            )
        } else if https_count == 0 {
            assert!(refs.iter().any(|r| r["id"] == "endpoint_0"));
            assert!(
                refs.iter()
                    .any(|r| r["id"] == "terrain_0" && r["source"] == "issued_dns_acquisition")
            );
            step_prop(
                &["previous_decision", "asset_0", "endpoint_0"],
                json!({"category":"validate","references":["asset_0","endpoint_0"],"manifest_reference":"https-head-v1","dependency_refresh":"dns_once_if_stale"}),
            )
        } else {
            let h0 = refs.iter().find(|r| r["id"] == "https_0").expect("https_0");
            assert_eq!(h0["source"], json!("issued_https_acquisition"));
            assert_eq!(h0["status_code"], json!(200));
            step_prop(&["previous_decision", "https_0", "mission"], defer_intent())
        };
        Reply::Length(200, serde_json::to_vec(&envelope(&prop)).unwrap())
    });

    let qref = Uuid::new_v4();
    let (e, c, run_id, _, mut runs, rec, rel) = start_cli_lab_run(qref);
    let ctrl = ControlDir::new();
    let renewals = Renewals::start(run_id);

    run_decision_phase(&mut runs, run_id, RunPhase::DecisionOne, &ctrl);
    runs.resolve_https_step_action(&rec, 1).unwrap();
    runs.protect_next_decision(run_id, 1).unwrap();
    runs.start_phase(run_id, RunPhase::AcquisitionOne).unwrap();
    runs.claim_phase(run_id, RunPhase::AcquisitionOne, &rel)
        .unwrap();
    let mut dns_store = PgAttemptDnsStore::new(broker(), 1, "dns");
    let mut dns_boundary = FakeDnsBoundary::new(vec![("198.51.100.10", 30)]);
    acquire_dns(&mut dns_store, &mut dns_boundary, run_id).unwrap();
    let step1 = runs.https_step(run_id, 1).unwrap();
    let iref = IssuedDnsReference::live(
        EngagementId(e),
        CampaignId(c),
        step1.dns_result_id.unwrap(),
        HTTPS_LAB_RUN_PROFILE,
    )
    .unwrap();
    let mut m_store = PgMissionStore::new(native::client("DW_TEST_DATABASE_URL"));
    let mut pts = PgTerrainStore::new(native::client("DW_TERRAIN_DATABASE_URL"));
    let (dns_ev, _, _, _) = pts.record_issued(&mut m_store, &iref).unwrap().unwrap();
    record_terrain_history(&mut native::client("DW_TRAJECTORY_DATABASE_URL"), &dns_ev).unwrap();
    runs.complete_phase(run_id, RunPhase::AcquisitionOne)
        .unwrap();

    run_decision_phase(&mut runs, run_id, RunPhase::DecisionTwo, &ctrl);
    runs.resolve_https_step_action(&rec, 2).unwrap();
    let refresh_d = format!("{:?}", runs.https_refresh_decision(run_id, 2).unwrap());
    assert!(
        refresh_d == "NotNeeded" || refresh_d == "Fresh",
        "{refresh_d}"
    );
    runs.protect_next_decision(run_id, 2).unwrap();
    runs.start_phase(run_id, RunPhase::AcquisitionTwo).unwrap();
    runs.claim_phase(run_id, RunPhase::AcquisitionTwo, &rel)
        .unwrap();
    let pg_now: i64 = broker()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    let offset = pg_now - now_ms();
    let mut h_store = PgHttpsAttemptStore::new(broker());
    let mut h_boundary = FakeHttpsBoundary { req: None, step: 0 };
    let mut clk = || Ok(now_ms() + offset + 10);
    acquire_with_clock(&mut h_store, &mut h_boundary, run_id, 2, &mut clk).unwrap();
    let head_res = runs
        .read_committed_https_result(run_id, 2)
        .unwrap()
        .unwrap();
    let step2 = runs.https_step(run_id, 2).unwrap();
    let mut phts = PgHttpsTerrainStore::new(&mut pts);
    let https_ev = phts
        .record_https(
            &mut m_store,
            &head_res,
            OperationId(step2.head_terrain_operation_id.unwrap()),
        )
        .unwrap();
    record_https_terrain_history(&mut native::client("DW_TRAJECTORY_DATABASE_URL"), &https_ev)
        .unwrap();
    runs.complete_phase(run_id, RunPhase::AcquisitionTwo)
        .unwrap();

    run_decision_phase(&mut runs, run_id, RunPhase::DecisionThree, &ctrl);
    runs.terminate_https_loop(run_id, 3, "deferred").unwrap();
    renewals.stop();
    runs.stop(run_id).unwrap();
    assert_eq!(server.count(), 3);

    let (ok_i, inspect) = cli_json(&["m1-run-inspect", "--run", &run_id.to_string()]);
    assert!(ok_i, "{inspect}");
    for _ in 0..2 {
        let (ok_r, recover) = cli_json(&["m1-run-recover", "--run", &run_id.to_string()]);
        assert!(ok_r, "{recover}");
        assert_eq!(recover, inspect);
    }
    for (k, exp) in [
        ("status", json!("deferred")),
        ("live_qualified", json!(true)),
        ("decision_count", json!(3)),
        ("decision_history_count", json!(3)),
        ("dns_effect_count", json!(1)),
        ("head_effect_count", json!(1)),
        ("terrain_history_count", json!(2)),
    ] {
        assert_eq!(inspect[k], exp);
    }
    assert_eq!(inspect["steps"][2]["disposition"], json!("deferred"));

    let mut admin = native::admin();
    let fix_rows: i64 = admin.query_one("SELECT (SELECT count(*) FROM execution.fixture_dns_attempts WHERE run_id=$1)+(SELECT count(*) FROM execution.fixture_https_attempts WHERE run_id=$1)+(SELECT count(*) FROM execution.fixture_https_results WHERE run_id=$1)", &[&run_id]).unwrap().get(0);
    let leaks: i64 = admin.query_one("SELECT count(*) FROM cognition.reasoning_decisions WHERE (e, c) = ($1, $2) AND payload::text ~ 'BEGIN CERTIFICATE|raw_body|x-lab-secret'", &[&e, &c]).unwrap().get(0);
    assert_eq!(
        (fix_rows, leaks),
        (0, 0),
        "live LAB run must not write fixture tables or leak raw secrets"
    );
}
