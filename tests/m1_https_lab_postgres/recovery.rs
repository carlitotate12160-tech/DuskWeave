use crate::admission::{broker, db_err, start_lab_run};
use crate::lab_support;
use crate::lineage::{commit_synthetic_step_decision, drive_step1_live_dns};
use crate::terrain::terrain_support::native;
use duskweave::m1_effect_control::authority::RestrictedWithdrawal;
use duskweave::m1_inference_profile::{HTTPS_LAB_IPV4, HTTPS_LAB_RUN_PROFILE};
use duskweave::m1_run::RunPhase;
use duskweave::m1_terrain_issued::IssuedDnsReference;
use duskweave::mission::{CampaignId, EngagementId, OperationId, OperatorRef};
use duskweave::postgres_m1_effect::PgStopWriter;
use duskweave::postgres_m1_terrain::PgTerrainStore;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_terrain_history::record_terrain_history;
use duskweave::withdrawal::{WithdrawalReason, WithdrawalRequest};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

#[ignore]
#[test]
fn protected_capacity_unknown_and_stop_release_no_stranded_holds() {
    let _guard = native::qualified_existing_db();
    let qref = Uuid::new_v4();
    let (e, c, run_id, reg, mut runs, rec) = start_lab_run(qref);
    let renew = Renewals::start(run_id);

    commit_synthetic_step_decision(
        &mut runs,
        &rec,
        &reg,
        1,
        json!({
            "category": "observe_more",
            "references": ["mission", "asset_0"],
            "manifest_reference": "dns_udp",
            "view_request": null,
            "dependency_refresh": "none"
        }),
    );
    runs.protect_next_decision(run_id, 1).unwrap();
    let mut b = broker();
    let snap: serde_json::Value = b
        .query_one(
            "SELECT execution.inference_capacity_snapshot($1,$2)",
            &[&e, &c],
        )
        .unwrap()
        .get(0);
    assert!(snap["charged"]["calls"].as_i64().unwrap() >= 2);
    runs.start_phase(run_id, RunPhase::AcquisitionOne).unwrap();
    runs.claim_phase(
        run_id,
        RunPhase::AcquisitionOne,
        &lab_support::sample_release_sha(),
    )
    .unwrap();

    let dns_rec: serde_json::Value = b
        .query_one(
            "SELECT execution.live_https_loop_dns_reserve($1,1,'dns')",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    let att_dns = Uuid::parse_str(dns_rec["request"]["attempt"].as_str().unwrap()).unwrap();
    let g_dns = dns_rec["request"]["session_generation"].as_i64().unwrap();
    let dl_dns: i64 = b
        .query_one(
            "SELECT execution.live_attempt_may_start($1,$2)",
            &[&att_dns, &g_dns],
        )
        .unwrap()
        .get(0);
    let dns_res = json!({
        "schema": "m1_dns_ipc_v1", "kind": "RESULT", "attempt": att_dns,
        "session_generation": g_dns, "sequence": 3, "status": "completed",
        "observed_at_ms": dl_dns - 2000, "rcode": 0, "truncated": false,
        "complete": true, "withheld_aliases": 0,
        "answers": [{"ipv4": HTTPS_LAB_IPV4, "ttl_seconds": 30}], "aliases": []
    });
    b.execute(
        "SELECT execution.live_attempt_dns_result($1,$2,$3)",
        &[&att_dns, &g_dns, &dns_res],
    )
    .unwrap();
    b.execute(
        "SELECT execution.live_attempt_stop($1,$2,true)",
        &[&att_dns, &g_dns],
    )
    .unwrap();

    runs.mark_https_effect_unknown(run_id, 1, "dns", "live_dns_unknown")
        .unwrap();
    runs.release_protected_capacity(run_id, 1, "unknown")
        .unwrap();
    renew.stop();
    let _ = runs.stop(run_id);

    let mut a = native::admin();
    let stranded: i64 = a
        .query_one(
            "SELECT count(*) FROM execution.https_loop_steps WHERE run_id=$1 AND protection_state='protected'",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    assert_eq!(stranded, 0);
}

#[ignore]
#[test]
fn stop_writer_withdrawal_blocks_live_reserve_and_ready_gates() {
    let _guard = native::qualified_existing_db();
    let qref = Uuid::new_v4();
    let (e, c, run_id, reg, mut runs, _) = start_lab_run(qref);
    let op_ref = Uuid::parse_str(reg["operator_ref"].as_str().unwrap()).unwrap();

    let mut writer = PgStopWriter::scoped(native::client("DW_STOP_WRITER_DATABASE_URL"));
    let withdrawal = WithdrawalRequest {
        engagement_id: EngagementId(e),
        campaign_id: CampaignId(c),
        operator_ref: OperatorRef(op_ref),
        expected_mission_revision: 1,
        reason: WithdrawalReason::OperatorRequested,
    };
    let ev = RestrictedWithdrawal::withdraw(
        &mut writer,
        &withdrawal,
        OperationId(Uuid::new_v4()),
        false,
    )
    .unwrap()
    .expect("withdrawal event emitted");
    assert_eq!(ev.request, withdrawal);

    let mut b = broker();
    let err = b
        .query_one(
            "SELECT execution.live_https_loop_dns_reserve($1,1,'dns')",
            &[&run_id],
        )
        .unwrap_err();
    let msg = db_err(&err);
    assert!(
        msg.contains("run_not_current") || msg.contains("live_authority_withdrawn"),
        "expected withdrawal refusal: {msg}"
    );
    let head_err = b
        .query_one(
            "SELECT execution.live_https_loop_head_reserve($1,1)",
            &[&run_id],
        )
        .unwrap_err();
    let hmsg = db_err(&head_err);
    assert!(
        hmsg.contains("run_not_current") || hmsg.contains("live_authority_withdrawn"),
        "expected head reserve withdrawal refusal: {hmsg}"
    );
    assert!(
        b.query_one(
            "SELECT execution.live_attempt_head_phase($1,1,'tcp')",
            &[&Uuid::new_v4()]
        )
        .is_err()
    );
    let _ = runs.stop(run_id);
}

pub(crate) struct FakeDnsBoundary {
    pub answers: Vec<(&'static str, u32)>,
    pub req: Option<(Uuid, u64, i64)>,
}
impl FakeDnsBoundary {
    pub(crate) fn new(answers: Vec<(&'static str, u32)>) -> Self {
        Self { answers, req: None }
    }
}
impl duskweave::m1_dns::DnsBoundary for FakeDnsBoundary {
    fn ready(&mut self, req: &duskweave::m1_dns::DnsRequest) -> duskweave::Res<()> {
        self.req = Some((req.attempt, req.session_generation, req.deadline_at_ms));
        Ok(())
    }
    fn start(&mut self, req: &duskweave::m1_dns::DnsRequest, dl: i64) -> duskweave::Res<()> {
        let started = (dl - 2000).max(1);
        self.req = Some((req.attempt, req.session_generation, started));
        Ok(())
    }
    fn result(&mut self) -> duskweave::Res<Option<duskweave::m1_dns::DnsResult>> {
        let (attempt, session_generation, obs_ms) =
            self.req.expect("start sets attempt and generation");
        let answers = self
            .answers
            .iter()
            .map(|(ip, ttl)| json!({"ipv4": ip, "ttl_seconds": ttl}))
            .collect::<Vec<_>>();
        Ok(Some(
            serde_json::from_value(json!({
                "schema": "m1_dns_ipc_v1", "kind": "RESULT",
                "attempt": attempt, "session_generation": session_generation, "sequence": 3,
                "status": "completed", "observed_at_ms": obs_ms,
                "rcode": 0, "truncated": false, "complete": true, "withheld_aliases": 0,
                "answers": answers, "aliases": []
            }))
            .unwrap(),
        ))
    }
    fn renew(&mut self, _: &duskweave::m1_dns::DnsRequest, _: i64) -> duskweave::Res<()> {
        Ok(())
    }
    fn acknowledge(&mut self, _: &duskweave::m1_dns::DnsRequest) -> duskweave::Res<bool> {
        Ok(true)
    }
    fn stop(&mut self) -> duskweave::Res<bool> {
        Ok(true)
    }
}

pub(crate) struct Renewals {
    stop: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<String>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Renewals {
    pub(crate) fn start(run: Uuid) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let failure = Arc::new(Mutex::new(None));
        let (s, f) = (stop.clone(), failure.clone());
        let worker = std::thread::spawn(move || {
            let mut runs = duskweave::postgres_m1_run::PgRunStore::new(broker());
            while !s.load(Ordering::SeqCst) {
                if let Err(e) = runs.renew(run) {
                    *f.lock().unwrap() = Some(format!("{e:?}"));
                    return;
                }
                std::thread::sleep(Duration::from_millis(350));
            }
        });
        Self {
            stop,
            failure,
            worker: Some(worker),
        }
    }
    pub(crate) fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
        assert!(self.failure.lock().unwrap().take().is_none());
    }
}
impl Drop for Renewals {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

#[ignore]
#[test]
fn live_https_loop_refresh_drift_and_pre_tls_failures() {
    let _guard = native::qualified_existing_db();
    let (e, c, run_id, reg, mut runs, rec) = start_lab_run(Uuid::new_v4());
    let renew = Renewals::start(run_id);
    let mut b = broker();
    let _ = drive_step1_live_dns(e, c, run_id, &reg, &mut runs, &rec, 2);
    std::thread::sleep(Duration::from_secs(3));
    commit_synthetic_step_decision(
        &mut runs,
        &rec,
        &reg,
        2,
        json!({"category":"validate","references":["asset_0","endpoint_0"],"manifest_reference":"https-head-v1","dependency_refresh":"dns_once_if_stale"}),
    );
    let r_needed = format!("{:?}", runs.https_refresh_decision(run_id, 2).unwrap());
    assert!(
        r_needed.contains("Refresh") || r_needed.contains("Needed"),
        "{r_needed}"
    );
    runs.protect_next_decision(run_id, 2).unwrap();
    runs.start_phase(run_id, RunPhase::AcquisitionTwo).unwrap();
    runs.claim_phase(
        run_id,
        RunPhase::AcquisitionTwo,
        &lab_support::sample_release_sha(),
    )
    .unwrap();
    let dns_ref: Value = b
        .query_one(
            "SELECT execution.live_https_loop_dns_reserve($1,2,'dns_refresh')",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    let att_r = Uuid::parse_str(dns_ref["request"]["attempt"].as_str().unwrap()).unwrap();
    let g_r = dns_ref["request"]["session_generation"].as_i64().unwrap();
    let dl_r: i64 = b
        .query_one(
            "SELECT execution.live_attempt_may_start($1,$2)",
            &[&att_r, &g_r],
        )
        .unwrap()
        .get(0);
    let res_drift = json!({
        "schema":"m1_dns_ipc_v1","kind":"RESULT","attempt":att_r,"session_generation":g_r,
        "sequence":3,"status":"completed","observed_at_ms":dl_r - 2000,"rcode":0,"truncated":false,
        "complete":true,"withheld_aliases":0,"answers":[{"ipv4":"198.51.100.99","ttl_seconds":30}],"aliases":[]
    });
    b.execute(
        "SELECT execution.live_attempt_dns_result($1,$2,$3)",
        &[&att_r, &g_r, &res_drift],
    )
    .unwrap();
    b.execute(
        "SELECT execution.live_attempt_stop($1,$2,true)",
        &[&att_r, &g_r],
    )
    .unwrap();
    let s2 = runs.https_step(run_id, 2).unwrap();
    let iref2 = IssuedDnsReference::live(
        EngagementId(e),
        CampaignId(c),
        s2.refresh_result_id.unwrap(),
        HTTPS_LAB_RUN_PROFILE,
    )
    .unwrap();
    let mut m_store = PgMissionStore::new(native::client("DW_TEST_DATABASE_URL"));
    let mut pts = PgTerrainStore::new(native::client("DW_TERRAIN_DATABASE_URL"));
    let (ev2, _, _, _) = pts.record_issued(&mut m_store, &iref2).unwrap().unwrap();
    record_terrain_history(&mut native::client("DW_TRAJECTORY_DATABASE_URL"), &ev2).unwrap();
    assert!(
        b.query_one(
            "SELECT execution.live_https_loop_head_reserve($1,2)",
            &[&run_id]
        )
        .is_err()
    );
    renew.stop();
    let _ = runs.stop(run_id);
}

fn spawn_acquisition_phase(run_id: Uuid, phase: RunPhase) -> (std::process::ExitStatus, String) {
    use std::io::Read;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let ctrl = std::env::temp_dir().join(format!("dw-rec-ctrl-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&ctrl).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ctrl, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_duskweave"));
    cmd.env_clear()
        .env("DW_M1_RUN_CONTROL_ROOT", &ctrl)
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
            cmd.env(key, v);
        }
    }
    let mut child = cmd
        .args([
            "m1-run-phase",
            "--run",
            &run_id.to_string(),
            "--phase",
            phase.name(),
            "--timing-profile",
            "reasoning-bounded-pro-v2",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let _keepalive = child.stdin.take().unwrap();
    let st = child.wait().unwrap();
    let (mut out, mut err) = (String::new(), String::new());
    let _ = child.stdout.take().unwrap().read_to_string(&mut out);
    let _ = child.stderr.take().unwrap().read_to_string(&mut err);
    let _ = std::fs::remove_dir_all(&ctrl);
    (st, format!("out={out} err={err}"))
}

#[ignore]
#[test]
fn live_https_loop_phase_cli_routes_dns_blocked_refresh_and_head() {
    use crate::lineage::drive_step1_live_dns_with_release;
    let _guard = native::qualified_existing_db();
    let (_e1, _c1, run1, reg1, mut runs, rec1, _) =
        crate::episode::start_cli_lab_run(Uuid::new_v4());
    let r1 = Renewals::start(run1);
    commit_synthetic_step_decision(
        &mut runs,
        &rec1,
        &reg1,
        1,
        json!({"category":"observe_more","references":["mission","asset_0"],"manifest_reference":"dns_udp","view_request":null,"dependency_refresh":"none"}),
    );
    runs.protect_next_decision(run1, 1).unwrap();
    runs.start_phase(run1, RunPhase::AcquisitionOne).unwrap();
    assert!(
        !spawn_acquisition_phase(run1, RunPhase::AcquisitionOne)
            .0
            .success()
    );
    assert_eq!(runs.https_step(run1, 1).unwrap().status, "unknown");
    r1.stop();
    let _ = runs.stop(run1);

    let (e2, c2, run2, reg2, mut runs2, rec2, rel2) =
        crate::episode::start_cli_lab_run(Uuid::new_v4());
    let r2 = Renewals::start(run2);
    drive_step1_live_dns_with_release(e2, c2, run2, &reg2, &mut runs2, &rec2, &rel2, 2);
    commit_synthetic_step_decision(
        &mut runs2,
        &rec2,
        &reg2,
        2,
        json!({"category":"validate","references":["asset_0","endpoint_0"],"manifest_reference":"https-head-v1","dependency_refresh":"none"}),
    );
    std::thread::sleep(Duration::from_millis(2200));
    runs2.protect_next_decision(run2, 2).unwrap();
    runs2.start_phase(run2, RunPhase::AcquisitionTwo).unwrap();
    let (st2, err2) = spawn_acquisition_phase(run2, RunPhase::AcquisitionTwo);
    assert!(st2.success(), "run2 stderr: {err2}");
    runs2
        .complete_phase(run2, RunPhase::AcquisitionTwo)
        .unwrap();
    r2.stop();
    let _ = runs2.stop(run2);

    let (e3, c3, run3, reg3, mut runs3, rec3, rel3) =
        crate::episode::start_cli_lab_run(Uuid::new_v4());
    let r3 = Renewals::start(run3);
    drive_step1_live_dns_with_release(e3, c3, run3, &reg3, &mut runs3, &rec3, &rel3, 60);
    commit_synthetic_step_decision(
        &mut runs3,
        &rec3,
        &reg3,
        2,
        json!({"category":"validate","references":["asset_0","endpoint_0"],"manifest_reference":"https-head-v1","dependency_refresh":"none"}),
    );
    runs3.protect_next_decision(run3, 2).unwrap();
    runs3.start_phase(run3, RunPhase::AcquisitionTwo).unwrap();
    assert!(
        !spawn_acquisition_phase(run3, RunPhase::AcquisitionTwo)
            .0
            .success()
    );
    assert_eq!(runs3.https_step(run3, 2).unwrap().status, "unknown");
    r3.stop();
    let _ = runs3.stop(run3);
}
