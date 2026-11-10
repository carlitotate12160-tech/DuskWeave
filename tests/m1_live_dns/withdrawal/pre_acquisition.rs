//! Pre-acquisition, mid-D1 and post-defer physical withdrawal cases on Linux.
#![cfg(target_os = "linux")]

use super::super::host::Host;
use super::super::lab;
use super::super::sdk_fixture::{Reply, Server};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Releases the held fixture reply on exit, including unwinding; the fixture's
/// own stop flag and bound remain the second, independent release path.
struct HeldReply(Arc<AtomicBool>);
impl Drop for HeldReply {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn withdraw_cli(
    host: &mut Host,
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
    let receipt = Host::receipt(&output);
    (output, json!({"receipt": receipt, "operation": operation}))
}

fn admin_session_count(host: &mut Host, e: Uuid, c: Uuid) -> i64 {
    host.admin
        .query_one(
            "SELECT count(*) FROM execution.live_effect_sessions WHERE e=$1 AND c=$2",
            &[&e, &c],
        )
        .unwrap()
        .get(0)
}

#[test]
#[ignore = "requires the owned installed release and restricted SQL owners; no session exists yet"]
fn withdrawal_before_any_run_commits_durably_and_recovers_key_free() {
    let mut host = Host::new();
    let request = lab::registered(&mut host, "seed.duskweave.test");
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

#[test]
#[ignore = "requires the owned installed release, resolver and restricted SQL owners"]
fn withdrawal_during_first_decision_stays_durable_and_stops_the_run() {
    // The first fixture request is the witnessed D1 boundary: its reply is held
    // so withdrawal lands mid-decision, before any provider answer or effect.
    let mut host = Host::new();
    host.start_resolver();
    let hold = Arc::new(AtomicBool::new(false));
    let _release = HeldReply(hold.clone());
    let server = Server::lab(move |incoming| {
        let body: serde_json::Value = serde_json::from_slice(&incoming.body).unwrap();
        lab::pack_in(&body).expect("witnessed D1 request pack");
        Reply::Hold(hold.clone())
    });
    let request = lab::registered(&mut host, "seed.duskweave.test");
    let file = host.write(&request);
    let mut parent = host.spawn(&[
        "m1-run",
        "--input",
        file.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-v1",
    ]);
    server.await_count(1);
    assert!(
        parent.try_wait().unwrap().is_none(),
        "parent still active at the witnessed D1 request"
    );
    {
        let wire = server.wire.lock().unwrap();
        assert_eq!(wire.write_complete, 0, "held D1 reply was never written");
        assert_eq!(wire.write_failed, 0, "held D1 reply did not fail");
    }
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
    {
        let wire = server.wire.lock().unwrap();
        assert_eq!(
            wire.write_complete, 0,
            "held D1 reply still unwritten after parent termination"
        );
        assert_eq!(wire.write_failed, 0);
    }
    assert_eq!(server.count(), 1, "exactly one fixture generation request");
    assert_eq!(host.count("dns_udp"), 0, "zero DNS questions");
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

#[test]
#[ignore = "requires the owned installed release, resolver and restricted SQL owners"]
fn withdrawal_after_first_decision_defer_stays_durable() {
    // After a D1 defer the campaign is already terminal; withdrawal still commits.
    let mut host = Host::new();
    host.start_resolver();
    let _server = lab::server_with("defer");
    let request = lab::registered(&mut host, "seed.duskweave.test");
    let (output2, receipt2) = lab::run(&mut host, &request);
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
