use crate::admission::{broker, lab_run_request, register_lab_campaign, seed_lab_qual};
use crate::genai::{Reply, Server, envelope};
use crate::lab_support;
use crate::recovery::{FakeDnsBoundary, Renewals};
use crate::terrain::terrain_support::native;
use duskweave::m1_effect_control::acquire as acquire_dns;
use duskweave::m1_effect_control::https::{HttpsBoundary, HttpsEvent};
use duskweave::m1_https::{HttpsIpcRequest, HttpsOutcome, HttpsPhase};
use duskweave::m1_inference_profile::HTTPS_LAB_RUN_PROFILE;
use duskweave::m1_run::{RunPhase, RunRecord};
use duskweave::m1_terrain_issued::IssuedDnsReference;
use duskweave::mission::{CampaignId, EngagementId};
use duskweave::postgres_m1_effect::https_loop::PgAttemptDnsStore;
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::postgres_m1_terrain::PgTerrainStore;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_terrain_history::record_terrain_history;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(crate) fn pg_now_ms() -> i64 {
    broker()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .unwrap()
        .get(0)
}

pub(crate) fn drive_d1_dns(
    e: Uuid,
    c: Uuid,
    run_id: Uuid,
    runs: &mut PgRunStore,
    rec: &RunRecord,
    rel: &str,
    ctrl: &ControlDir,
) -> (PgMissionStore, PgTerrainStore) {
    run_decision_phase(runs, run_id, RunPhase::DecisionOne, ctrl);
    runs.resolve_https_step_action(rec, 1).unwrap();
    runs.protect_next_decision(run_id, 1).unwrap();
    runs.start_phase(run_id, RunPhase::AcquisitionOne).unwrap();
    runs.claim_phase(run_id, RunPhase::AcquisitionOne, rel)
        .unwrap();
    let mut dns_s = PgAttemptDnsStore::new(broker(), 1, "dns");
    let mut dns_b = FakeDnsBoundary::new(vec![("198.51.100.10", 30)]);
    acquire_dns(&mut dns_s, &mut dns_b, run_id).unwrap();
    let dns_id = runs.https_step(run_id, 1).unwrap().dns_result_id.unwrap();
    let iref = IssuedDnsReference::live(
        EngagementId(e),
        CampaignId(c),
        dns_id,
        HTTPS_LAB_RUN_PROFILE,
    )
    .unwrap();
    let mut m_store = PgMissionStore::new(native::client("DW_TEST_DATABASE_URL"));
    let mut pts = PgTerrainStore::new(native::client("DW_TERRAIN_DATABASE_URL"));
    let (dns_ev, _, _, _) = pts.record_issued(&mut m_store, &iref).unwrap().unwrap();
    record_terrain_history(&mut native::client("DW_TRAJECTORY_DATABASE_URL"), &dns_ev).unwrap();
    runs.complete_phase(run_id, RunPhase::AcquisitionOne)
        .unwrap();
    (m_store, pts)
}

pub(crate) struct ControlDir(pub(crate) PathBuf);
impl ControlDir {
    pub(crate) fn new() -> Self {
        let stamp = std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos();
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
            let script = "$ErrorActionPreference='Stop';$s=[Security.Principal.WindowsIdentity]::GetCurrent().User;$a=New-Object Security.AccessControl.DirectorySecurity;$a.SetOwner($s);$a.SetAccessRuleProtection($true,$false);$a.AddAccessRule((New-Object Security.AccessControl.FileSystemAccessRule($s,'FullControl','ContainerInherit,ObjectInherit','None','Allow')));Set-Acl -LiteralPath $env:DW_M1_RUN_CONTROL_ROOT -AclObject $a";
            let status = Command::new(
                PathBuf::from(&system).join("System32/WindowsPowerShell/v1.0/powershell.exe"),
            )
            .env_clear()
            .env("SystemRoot", system)
            .env("DW_M1_RUN_CONTROL_ROOT", &root)
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .status()
            .unwrap();
            assert!(status.success());
        }
        Self(root)
    }
}
impl Drop for ControlDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn pack_in(value: &Value) -> Option<Value> {
    match value {
        Value::String(s) => serde_json::from_str::<Value>(s)
            .ok()
            .filter(|p| p["version"] == duskweave::m1_https::VERSION_HTTPS_LOOP),
        Value::Array(a) => a.iter().find_map(pack_in),
        Value::Object(o) => o.values().find_map(pack_in),
        _ => None,
    }
}

pub(crate) struct FakeHttpsBoundary {
    req: Option<HttpsIpcRequest>,
    step: u8,
    outcome: HttpsOutcome,
    observed_at_ms: Option<i64>,
}
impl FakeHttpsBoundary {
    pub(crate) fn with_observed_ms(outcome: HttpsOutcome, obs_ms: i64) -> Self {
        Self {
            req: None,
            step: 0,
            outcome,
            observed_at_ms: (obs_ms > 0).then_some(obs_ms),
        }
    }
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
        let obs_ms = self
            .observed_at_ms
            .unwrap_or_else(|| req.deadline_at_ms - i64::from(req.max_duration_ms));
        let (out, code) = match (self.step, self.outcome) {
            (1, HttpsOutcome::ConnectionFailed) => (HttpsOutcome::ConnectionFailed, 0),
            (1, _) => return Ok(Some(HttpsEvent::Ready(HttpsPhase::Tls))),
            (2, HttpsOutcome::HeadResponse) => {
                return Ok(Some(HttpsEvent::Ready(HttpsPhase::Head)));
            }
            (3, HttpsOutcome::HeadResponse) => (HttpsOutcome::HeadResponse, 200),
            (2, out) => (out, 0),
            _ => return Ok(None),
        };
        Ok(Some(HttpsEvent::Result(
            lab_support::sample_https_safe_result(req, out, obs_ms, code, "none", None),
        )))
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
    let pass = [
        "DW_INFERENCE_BROKER_DATABASE_URL",
        "DW_COGNITION_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL",
        "DW_TERRAIN_DATABASE_URL",
        "SystemRoot",
        "LLVM_PROFILE_FILE",
    ];
    c.envs(
        pass.into_iter()
            .filter_map(|k| std::env::var_os(k).map(|v| (k, v))),
    );
    c
}

pub(crate) fn cli_json(args: &[&str]) -> (bool, Value) {
    let out = cli_cmd().args(args).output().unwrap();
    let txt = String::from_utf8_lossy(&out.stdout);
    let parsed = txt
        .lines()
        .rev()
        .find(|l| l.starts_with('{'))
        .map(|l| serde_json::from_str(l).unwrap())
        .unwrap_or_else(
            || json!({"code": out.status.code(), "raw": txt.trim(), "err": String::from_utf8_lossy(&out.stderr).trim()}),
        );
    (out.status.success(), parsed)
}

pub(crate) fn run_decision_phase(
    runs: &mut PgRunStore,
    run_id: Uuid,
    phase: RunPhase,
    ctrl: &ControlDir,
) {
    runs.start_phase(run_id, phase).unwrap();
    let run_s = run_id.to_string();
    let mut child = cli_cmd()
        .env("DW_M1_RUN_CONTROL_ROOT", &ctrl.0)
        .env("DW_M1_RUN_PROVIDER_DECISION", "1")
        .args([
            "m1-run-phase",
            "--run",
            &run_s,
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
        "m1-run-phase {} failed ({status:?}): stdout={stdout}; stderr={}",
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
    let (e, c, run_id) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
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
    lab_support::sha256_hex(&std::fs::read(env!("CARGO_BIN_EXE_duskweave")).unwrap())
}

pub(crate) type CliRun = (
    Uuid,
    Uuid,
    Uuid,
    Value,
    PgRunStore,
    duskweave::m1_run::RunRecord,
    String,
);

pub(crate) fn start_cli_lab_run(qref: Uuid) -> CliRun {
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
    let (e, c, run_id) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let reg = crate::admission::base_registration_json(e, c, qref, &release);
    register_lab_campaign(&reg).unwrap();
    let req_val = lab_run_request(&reg, run_id, qref, &release);
    let req = duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&req_val).unwrap()).unwrap();
    let mut runs = PgRunStore::new(broker());
    assert!(runs.start(&req, &release).unwrap());
    let rec = runs.inspect(run_id).unwrap();
    (e, c, run_id, reg, runs, rec, release)
}

pub(crate) fn step_prop(refs: &[&str], next: Value) -> Value {
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

pub(crate) fn assert_inspect_counts(v: &Value, st: &str, dec: u64, dns: u64, head: u64) {
    assert_eq!(
        (
            &v["status"],
            &v["live_qualified"],
            &v["decision_count"],
            &v["dns_effect_count"],
            &v["head_effect_count"]
        ),
        (
            &json!(st),
            &json!(true),
            &json!(dec),
            &json!(dns),
            &json!(head)
        )
    );
}

#[ignore]
#[test]
fn cli_m1_run_phase_d1_defer_and_inspect_recover_on_schema0038() {
    let _guard = native::qualified_existing_db();
    let server = Server::lab(|req| {
        let wire: Value = serde_json::from_slice(&req.body).unwrap();
        assert!(pack_in(&wire).is_some());
        Reply::Length(
            200,
            serde_json::to_vec(&envelope(&step_prop(&["mission"], defer_intent()))).unwrap(),
        )
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
        assert_inspect_counts(&v, "deferred", 1, 0, 0);
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
    crate::feedback::drive_d1_d2(
        (e, c, run_id, &rel),
        &mut runs,
        &rec,
        &ctrl,
        (HttpsOutcome::HeadResponse, None, None),
        true,
    );

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
    assert_inspect_counts(&inspect, "deferred", 3, 1, 1);
    assert_eq!(
        (
            &inspect["decision_history_count"],
            &inspect["terrain_history_count"],
            &inspect["steps"][2]["disposition"]
        ),
        (&json!(3), &json!(2), &json!("deferred"))
    );

    let mut admin = native::admin();
    let fix_rows: i64 = admin.query_one("SELECT (SELECT count(*) FROM execution.fixture_dns_attempts WHERE run_id=$1)+(SELECT count(*) FROM execution.fixture_https_attempts WHERE run_id=$1)+(SELECT count(*) FROM execution.fixture_https_results WHERE run_id=$1)", &[&run_id]).unwrap().get(0);
    let leaks: i64 = admin.query_one("SELECT count(*) FROM cognition.reasoning_decisions WHERE (e, c) = ($1, $2) AND payload::text ~ 'BEGIN CERTIFICATE|raw_body|x-lab-secret'", &[&e, &c]).unwrap().get(0);
    assert_eq!(
        (fix_rows, leaks),
        (0, 0),
        "live LAB run must not write fixture tables or leak raw secrets"
    );
}
