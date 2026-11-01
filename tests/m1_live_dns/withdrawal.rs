//! Typed coordination witnesses; physical CLI/host cases are qualified separately.
use duskweave::m1_effect_control::authority::*;
use duskweave::mission::{CampaignId, EngagementId, OperationId, OperatorRef};
use duskweave::withdrawal::{MissionAuthorityWithdrawn, WithdrawalReason, WithdrawalRequest};
use duskweave::{Fail, Res};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
struct Host(Arc<Mutex<Vec<&'static str>>>);
impl LocalStop for Host {
    fn stop(&mut self, _: StopTarget) -> Res<StopDisposition> {
        self.0.lock().unwrap().push("host_stop");
        Ok(StopDisposition {
            stop_latched: true,
            disposal_confirmed: false,
        })
    }
}
struct Writer(Arc<Mutex<Vec<&'static str>>>);
impl RestrictedWithdrawal for Writer {
    fn withdraw(
        &mut self,
        _: &WithdrawalRequest,
        _: OperationId,
        _: bool,
    ) -> Res<Option<MissionAuthorityWithdrawn>> {
        self.0.lock().unwrap().push("sql");
        Err(Fail::Store("commit_unknown"))
    }
}
fn request() -> WithdrawalRequest {
    WithdrawalRequest {
        engagement_id: EngagementId(Uuid::from_u128(1)),
        campaign_id: CampaignId(Uuid::from_u128(2)),
        operator_ref: OperatorRef(Uuid::from_u128(3)),
        expected_mission_revision: 1,
        reason: WithdrawalReason::OperatorRequested,
    }
}
#[test]
fn unavailable_database_keeps_local_latch_and_original_scope_after_host_stop() {
    let steps = Arc::new(Mutex::new(vec![]));
    let q = request();
    let operation = OperationId(Uuid::from_u128(4));
    let outcome = withdraw(
        &mut Host(steps.clone()),
        &mut Writer(steps.clone()),
        &q,
        operation,
        false,
    );
    assert_eq!(*steps.lock().unwrap(), vec!["host_stop", "sql"]);
    assert_eq!(
        outcome.local,
        StopDisposition {
            stop_latched: true,
            disposal_confirmed: false
        }
    );
    assert_eq!(outcome.durable, Err(Fail::Store("commit_unknown")));
    assert_eq!(q.campaign_id.0, Uuid::from_u128(2));
}
#[test]
fn withdrawal_recovery_reads_durable_owner_without_reissuing_host_work() {
    let steps = Arc::new(Mutex::new(vec![]));
    let _ = withdraw(
        &mut Host(steps.clone()),
        &mut Writer(steps.clone()),
        &request(),
        OperationId(Uuid::from_u128(4)),
        true,
    );
    assert_eq!(*steps.lock().unwrap(), vec!["sql"]);
}

#[cfg(target_os = "linux")]
struct DeploymentPatch(Vec<u8>);
#[cfg(target_os = "linux")]
impl DeploymentPatch {
    fn ineligible() -> Self {
        let path = "/etc/duskweave/m1/deployment.json";
        let original = std::fs::read(path).unwrap();
        let mut manifest: serde_json::Value = serde_json::from_slice(&original).unwrap();
        manifest["valid_until_ms"] = serde_json::json!(1);
        manifest["image_digest"] = serde_json::json!(format!("sha256:{}", "0".repeat(64)));
        std::fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        Self(original)
    }
}
#[cfg(target_os = "linux")]
impl Drop for DeploymentPatch {
    fn drop(&mut self) {
        std::fs::write("/etc/duskweave/m1/deployment.json", &self.0).unwrap();
    }
}
#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the owned installed core and protected deployment; SQL refusal is deliberate"]
fn expired_qualification_and_missing_image_cannot_disable_host_stop_before_database_lookup() {
    use super::host::Host;
    use serde_json::json;
    let mut host = Host::new();
    let e = host.id();
    let c = host.id();
    let op = host.id();
    let q = json!({"engagement_id":e,"campaign_id":c,"operator_ref":host.id(),
        "expected_mission_revision":1,"reason":"operator_requested"});
    let request = host.write(&q);
    let dsn = std::fs::read_to_string(&host.environment["DW_DATABASE_URL_FILE"]).unwrap();
    assert_eq!(dsn.matches("@127.0.0.1:5432/").count(), 1);
    let broken = host.private_bytes(dsn.replace("@127.0.0.1:5432/", "@127.0.0.1:1/").as_bytes());
    host.environment.insert(
        "DW_DATABASE_URL_FILE".into(),
        broken.to_str().unwrap().into(),
    );
    let _patch = DeploymentPatch::ineligible();
    let output = host.cli(&[
        "withdraw",
        "--operation",
        &op.to_string(),
        "--input",
        request.to_str().unwrap(),
        "--recover",
        "false",
    ]);
    let receipt = Host::receipt(&output);
    assert!(
        !output.status.success(),
        "database failure cannot become durable withdrawal"
    );
    assert_eq!(receipt["result"], "unknown");
    assert_eq!(receipt["operation"], json!(op));
    assert_eq!(receipt["scope"], json!([e, c]));
    assert_eq!(
        receipt["local_stop"]["stop_latched"], true,
        "authenticated host stop precedes even the failed first SQL connection"
    );
    assert!(
        std::path::Path::new(&format!(
            "/run/duskweave/launcher/campaign-stop-{}-{}",
            e.simple(),
            c.simple()
        ))
        .exists()
    );
    assert_eq!(receipt["continuation_blocked"], true);
    std::fs::write(
        host.report.join("withdrawal.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the owned installed release and isolated DNS LAB; one admitted unanswered query"]
fn in_flight_run_stop_is_latched_disposed_and_durably_stopped_without_refund() {
    use super::host::{Host, command};
    use serde_json::Value;
    use std::time::{Duration, Instant};
    let mut host = Host::new();
    host.start_resolver();
    let server = super::lab::server();
    let request = super::lab::registered(&mut host, "timeout.duskweave.test");
    let file = host.write(&request);
    let run = request["run_id"].as_str().unwrap();
    let mut parent = host.spawn(&[
        "m1-run",
        "--input",
        file.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-v1",
    ]);
    let end = Instant::now() + Duration::from_secs(8);
    while host.count("dns_udp") == 0 && Instant::now() < end {
        if parent.try_wait().unwrap().is_some() {
            let terminal = parent.output();
            panic!(
                "parent exited before real START: {}",
                String::from_utf8_lossy(&terminal.stdout)
            );
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        host.count("dns_udp"),
        1,
        "one released unanswered query before STOP"
    );
    let began = Instant::now();
    let stopped = host.cli(&["m1-run-stop", "--run", run]);
    let local: Value = String::from_utf8(stopped.stdout.clone())
        .unwrap()
        .lines()
        .filter_map(|s| serde_json::from_str::<Value>(s).ok())
        .find(|v| v.get("local_stop").is_some())
        .expect("correlated local STOP receipt");
    assert_eq!(local["local_stop"]["stop_latched"], true);
    assert!(
        began.elapsed() < Duration::from_millis(1000),
        "host STOP and CLI bookkeeping remain responsive"
    );
    assert!(
        parent
            .wait_until(Instant::now() + Duration::from_secs(3))
            .is_some(),
        "owned active run terminates"
    );
    let _terminal = parent.output();
    let run_id: Uuid = Uuid::parse_str(run).unwrap();
    let attempt: Uuid = host
        .admin
        .query_one(
            "SELECT attempt FROM execution.autonomous_runs WHERE run_id=$1",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    assert!(
        String::from_utf8(
            command(
                "/usr/bin/docker",
                &[
                    "ps",
                    "-a",
                    "--filter",
                    &format!("label=org.duskweave.attempt={attempt}"),
                    "--format",
                    "{{.ID}}"
                ]
            )
            .stdout
        )
        .unwrap()
        .trim()
        .is_empty(),
        "both owned workers are absent after STOP"
    );
    assert!(
        std::path::Path::new(&format!(
            "/run/duskweave/launcher/run-stop-{}",
            run_id.simple()
        ))
        .exists()
    );
    assert_eq!(host.count("dns_udp"), 1);
    assert_eq!(server.count(), 1);
    std::fs::write(host.report.join("stop-receipt.json"), &stopped.stdout).unwrap();
    assert!(
        stopped.status.success(),
        "normal run-stop must durably stop its own run: {}",
        String::from_utf8_lossy(&stopped.stdout)
    );
    let view = Host::receipt(&stopped);
    assert_eq!(view["status"], "stopped");
    let row = host
        .admin
        .query_one(
            "SELECT possibly_started FROM execution.live_effect_control WHERE run_id=$1",
            &[&run_id],
        )
        .unwrap();
    assert!(
        row.get::<_, bool>(0),
        "STOP never refunds the may-start charge"
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the owned schema-0030 b restricted stop-writer login"]
fn stop_writer_metadata_distinguishes_absence_without_granting_any_effect() {
    use super::host::Host;
    let mut host = Host::new();
    let dsn =
        std::fs::read_to_string(&host.environment["DW_STOP_WRITER_DATABASE_URL_FILE"]).unwrap();
    let mut stop =
        postgres::Client::connect(&dsn, postgres::NoTls).expect("restricted stop-writer login");
    let (e, c) = (host.id(), host.id());
    let generation: Option<i64> = stop
        .query_one("SELECT execution.live_stop_generation($1,$2)", &[&e, &c])
        .expect("stop-only metadata must return absence for an unbound campaign")
        .get(0);
    assert_eq!(generation, None);
    let error = stop
        .batch_execute("INSERT INTO execution.live_effect_sessions DEFAULT VALUES")
        .unwrap_err();
    assert_eq!(
        error.code(),
        Some(&postgres::error::SqlState::INSUFFICIENT_PRIVILEGE)
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the owned installed release, restricted SQL owners and one in-flight LAB query"]
fn normal_withdrawal_stops_first_uses_stop_writer_and_publishes_existing_history_contract() {
    use super::host::Host;
    use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
    use duskweave::withdrawal::WithdrawalStore;
    use serde_json::json;
    use std::time::{Duration, Instant};
    let mut host = Host::new();
    host.start_resolver();
    let server = super::lab::server();
    let request = super::lab::registered(&mut host, "timeout.duskweave.test");
    let file = host.write(&request);
    let mut parent = host.spawn(&[
        "m1-run",
        "--input",
        file.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-v1",
    ]);
    let end = Instant::now() + Duration::from_secs(8);
    while host.count("dns_udp") == 0 && Instant::now() < end {
        if parent.try_wait().unwrap().is_some() {
            panic!(
                "normal parent ended before START: {}",
                String::from_utf8_lossy(&parent.output().stdout)
            );
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(host.count("dns_udp"), 1);
    let q = json!({"engagement_id":request["context"]["mission"]["engagement_id"],
        "campaign_id":request["context"]["mission"]["campaign_id"],
        "operator_ref":request["context"]["mission"]["operator_ref"],"expected_mission_revision":1,
        "reason":"operator_requested"});
    let typed: WithdrawalRequest = serde_json::from_value(q.clone()).unwrap();
    let core = std::fs::read_to_string(&host.environment["DW_DATABASE_URL_FILE"]).unwrap();
    let mut old = PgMissionStore::new(postgres::Client::connect(&core, postgres::NoTls).unwrap());
    let mut allocator =
        PgAllocator::new(postgres::Client::connect(&core, postgres::NoTls).unwrap());
    assert!(
        old.withdraw(&typed, OperationId(host.id()), false, &mut allocator)
            .is_err(),
        "ordinary legacy Mission writer cannot mutate active live authority"
    );
    let withdrawal = host.write(&q);
    let operation = host.id();
    let output = host.cli(&[
        "withdraw",
        "--operation",
        &operation.to_string(),
        "--input",
        withdrawal.to_str().unwrap(),
        "--recover",
        "false",
    ]);
    let receipt = Host::receipt(&output);
    assert!(
        output.status.success(),
        "normal restricted withdrawal: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(receipt["result"], "durable");
    assert_eq!(receipt["history"], "completed");
    assert_eq!(receipt["local_stop"]["stop_latched"], true);
    assert!(
        parent
            .wait_until(Instant::now() + Duration::from_secs(3))
            .is_some()
    );
    let _terminal = parent.output();
    assert_eq!(host.count("dns_udp"), 1);
    assert_eq!(server.count(), 1);
    for _ in 0..2 {
        let recovered = host.cli(&[
            "withdraw",
            "--operation",
            &operation.to_string(),
            "--input",
            withdrawal.to_str().unwrap(),
            "--recover",
            "true",
        ]);
        assert!(recovered.status.success());
        let view = Host::receipt(&recovered);
        assert_eq!(view["contract"], receipt["contract"]);
        assert_eq!(view["history"], "completed");
        assert_eq!(host.count("dns_udp"), 1);
        assert_eq!(server.count(), 1);
    }
    std::fs::write(
        host.report.join("durable-withdrawal.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

#[cfg(target_os = "linux")]
fn withdraw_cli(
    host: &mut super::host::Host,
    e: Uuid,
    c: Uuid,
    operator: Uuid,
    operation: Uuid,
    recover: bool,
) -> (std::process::Output, serde_json::Value) {
    use serde_json::json;
    let q = json!({"engagement_id":e,"campaign_id":c,"operator_ref":operator,
        "expected_mission_revision":1,"reason":"operator_requested"});
    let file = host.write(&q);
    let output = host.cli(&[
        "withdraw",
        "--operation",
        &operation.to_string(),
        "--input",
        file.to_str().unwrap(),
        "--recover",
        if recover { "true" } else { "false" },
    ]);
    let receipt = super::host::Host::receipt(&output);
    (output, json!({"receipt": receipt, "operation": operation}))
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the owned installed release and restricted SQL owners; no session exists yet"]
fn withdrawal_before_any_run_commits_durably_and_recovers_key_free() {
    use super::host::Host;
    let mut host = Host::new();
    let request = super::lab::registered(&mut host, "seed.duskweave.test");
    let mission = &request["context"]["mission"];
    let e: Uuid = serde_json::from_value(mission["engagement_id"].clone()).unwrap();
    let c: Uuid = serde_json::from_value(mission["campaign_id"].clone()).unwrap();
    let operator: Uuid = serde_json::from_value(mission["operator_ref"].clone()).unwrap();
    // Before m1-run there is no effect session at all; durable withdrawal is
    // still owed through the fenced Mission path, never an absent-generation error.
    let operation = host.id();
    let (output, view) = withdraw_cli(&mut host, e, c, operator, operation, false);
    assert!(
        output.status.success(),
        "pre-run withdrawal: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(view["receipt"]["result"], "durable");
    assert_eq!(view["receipt"]["history"], "completed");
    assert_eq!(view["receipt"]["continuation_blocked"], true);
    assert_eq!(
        admin_session_count(&mut host, e, c),
        0,
        "no session was manufactured"
    );
    for _ in 0..2 {
        let (recovered, view2) = withdraw_cli(&mut host, e, c, operator, operation, true);
        assert!(recovered.status.success());
        assert_eq!(view2["receipt"]["contract"], view["receipt"]["contract"]);
        assert_eq!(view2["receipt"]["history"], "completed");
    }
    std::fs::write(
        host.report.join("pre-run-withdrawal.json"),
        serde_json::to_vec_pretty(&view["receipt"]).unwrap(),
    )
    .unwrap();
}

#[cfg(target_os = "linux")]
fn admin_session_count(host: &mut super::host::Host, e: Uuid, c: Uuid) -> i64 {
    host.admin
        .query_one(
            "SELECT count(*) FROM execution.live_effect_sessions WHERE e=$1 AND c=$2",
            &[&e, &c],
        )
        .unwrap()
        .get(0)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the owned installed release, resolver and restricted SQL owners"]
fn withdrawal_during_first_decision_stays_durable_and_stops_the_run() {
    use super::host::Host;
    use std::time::{Duration, Instant};
    // During D1, before any acquisition reservation.
    let mut host = Host::new();
    host.start_resolver();
    let _server = super::lab::server();
    let request = super::lab::registered(&mut host, "seed.duskweave.test");
    let file = host.write(&request);
    let mut parent = host.spawn(&[
        "m1-run",
        "--input",
        file.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-v1",
    ]);
    let mission = &request["context"]["mission"];
    let e: Uuid = serde_json::from_value(mission["engagement_id"].clone()).unwrap();
    let c: Uuid = serde_json::from_value(mission["campaign_id"].clone()).unwrap();
    let operator: Uuid = serde_json::from_value(mission["operator_ref"].clone()).unwrap();
    let run: Uuid = serde_json::from_value(request["run_id"].clone()).unwrap();
    let operation = host.id();
    let (output, view) = withdraw_cli(&mut host, e, c, operator, operation, false);
    assert!(
        output.status.success(),
        "mid-D1 withdrawal: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(view["receipt"]["result"], "durable");
    assert!(
        parent
            .wait_until(Instant::now() + Duration::from_secs(5))
            .is_some(),
        "owned run terminates on withdrawal"
    );
    let _terminal = parent.output();
    assert_eq!(
        host.admin
            .query_one(
                "SELECT count(*) FROM execution.live_dns_results WHERE run_id=$1",
                &[&run]
            )
            .unwrap()
            .get::<_, i64>(0),
        0,
        "no committed result can survive the withdrawal"
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the owned installed release, resolver and restricted SQL owners"]
fn withdrawal_after_first_decision_defer_stays_durable() {
    use super::host::Host;
    // After a D1 defer the campaign is already terminal; withdrawal still commits.
    let mut host = Host::new();
    host.start_resolver();
    let _server = super::lab::server_with("defer");
    let request = super::lab::registered(&mut host, "seed.duskweave.test");
    let (output2, receipt2) = super::lab::run(&mut host, &request);
    let mission = &request["context"]["mission"];
    let e: Uuid = serde_json::from_value(mission["engagement_id"].clone()).unwrap();
    let c: Uuid = serde_json::from_value(mission["campaign_id"].clone()).unwrap();
    let operator: Uuid = serde_json::from_value(mission["operator_ref"].clone()).unwrap();
    assert!(output2.status.success(), "deferred run completes");
    assert_eq!(receipt2["status"], "deferred");
    assert_eq!(receipt2["effect_count"], 0, "defer acquired nothing");
    let operation3 = host.id();
    let (output3, view3) = withdraw_cli(&mut host, e, c, operator, operation3, false);
    assert!(
        output3.status.success(),
        "post-defer withdrawal: {}",
        String::from_utf8_lossy(&output3.stdout)
    );
    assert_eq!(view3["receipt"]["result"], "durable");
}
