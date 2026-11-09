//! Durable pre-recovery witnesses and fresh key-free no-replay consumers.
#![cfg(target_os = "linux")]
#[path = "../support/database_wait_proxy.rs"]
mod wait_proxy;
use super::host::Host;
use super::sdk_fixture::{Reply, Server, envelope};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;
struct RestoreFile {
    path: std::path::PathBuf,
    bytes: Vec<u8>,
}
impl Drop for RestoreFile {
    fn drop(&mut self) {
        std::fs::write(&self.path, &self.bytes).unwrap();
    }
}
fn result_witness(host: &mut Host, run: Uuid) -> Value {
    let row=host.admin.query_one(
        "SELECT b.result,b.attempt,b.session_generation,b.publication_obligation,b.projection,x.phase,x.possibly_started,x.disposal_confirmed FROM execution.live_dns_results b JOIN execution.live_effect_control x USING(run_id) WHERE b.run_id=$1",&[&run])
        .expect("actual pre-recovery committed result");
    json!({"run_id":run,"result":row.get::<_,Uuid>(0),"attempt":row.get::<_,Uuid>(1),
        "generation":row.get::<_,i64>(2),"publication_obligation":row.get::<_,String>(3),
        "projection":row.get::<_,Value>(4),"phase":row.get::<_,String>(5),
        "possibly_started":row.get::<_,bool>(6),"disposal_confirmed":row.get::<_,bool>(7)})
}
#[test]
#[ignore = "requires the owned installed CLI and real result COMMIT response withheld by the private fixture relay"]
fn real_result_commit_then_ack_loss_recovers_same_publication_without_a_second_send() {
    let mut host = Host::new();
    host.start_resolver();
    let normal = std::fs::read(&host.environment["DW_INFERENCE_BROKER_DATABASE_URL_FILE"]).unwrap();
    let private = host.private_bytes(&normal);
    host.environment.insert(
        "DW_INFERENCE_BROKER_DATABASE_URL_FILE".into(),
        private.to_str().unwrap().into(),
    );
    let _restore = RestoreFile {
        path: private.clone(),
        bytes: normal.clone(),
    };
    let relay = wait_proxy::Relay::start("127.0.0.1:5432".parse().unwrap(), b"COMMIT");
    let source = String::from_utf8(normal.clone()).unwrap();
    assert_eq!(source.matches("@127.0.0.1:5432/").count(), 1);
    let routed = source.replace("@127.0.0.1:5432/", &format!("@127.0.0.1:{}/", relay.port()));
    let route = private.clone();
    let server = Server::lab(move |incoming| {
        let body: Value = serde_json::from_slice(&incoming.body).unwrap();
        let pack = super::lab::pack_in(&body).unwrap();
        assert_eq!(
            pack["predecessor"]["availability"], "absent",
            "recovery cannot generate D2"
        );
        // D1 already owns its connections. Only the later acquisition child reads this path.
        std::fs::write(&route, routed.as_bytes()).unwrap();
        Reply::Length(
            200,
            serde_json::to_vec(&envelope(&super::lab::proposal(&pack))).unwrap(),
        )
    });
    let request = super::lab::registered(&mut host, "seed.duskweave.test");
    let run: Uuid = serde_json::from_value(request["run_id"].clone()).unwrap();
    let file = host.write(&request);
    let mut parent = host.spawn(&[
        "m1-run",
        "--input",
        file.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-v1",
    ]);
    assert!(
        relay.observed(Duration::from_secs(10)),
        "actual COMMIT was forwarded"
    );
    assert!(
        relay.consumed(Duration::from_secs(2)),
        "server reported COMMIT, not ROLLBACK/error"
    );
    let before = result_witness(&mut host, run);
    assert_eq!(before["phase"], "result_recorded");
    assert_eq!(
        before["disposal_confirmed"], false,
        "ACK still withheld by the real commit response fault"
    );
    assert_eq!(before["publication_obligation"], "terrain.issued_dns.v2");
    assert_eq!(host.count("dns_udp"), 1);
    assert_eq!(server.count(), 1);
    std::fs::write(
        host.report.join("before-recovery.json"),
        serde_json::to_vec_pretty(&before).unwrap(),
    )
    .unwrap();
    drop(relay);
    std::fs::write(&private, &normal).unwrap();
    assert!(
        parent
            .wait_until(Instant::now() + Duration::from_secs(6))
            .is_some(),
        "lost commit response terminates normally supervised child"
    );
    let _terminal = parent.output();
    for _ in 0..2 {
        let output = host.cli(&["m1-run-recover", "--run", &run.to_string()]);
        assert!(output.status.success(), "fresh key-free recovery");
        let view = Host::receipt(&output);
        assert_eq!(view["effect_count"], 1);
        assert_eq!(view["terrain_history_count"], 1);
        let after = result_witness(&mut host, run);
        for field in [
            "run_id",
            "result",
            "attempt",
            "generation",
            "projection",
            "publication_obligation",
        ] {
            assert_eq!(after[field], before[field], "immutable committed {field}");
        }
        assert_eq!(
            after["possibly_started"], true,
            "recovery never refunds uncertainty"
        );
        assert_eq!(host.count("dns_udp"), 1);
        assert_eq!(server.count(), 1);
    }
}
#[test]
#[ignore = "requires actual TTL expiry after D2 fixture request release, owned SQL/history and two fresh recoveries"]
fn ttl_expiry_after_d2_release_records_stale_proposal_and_recovery_never_resends() {
    let mut host = Host::new();
    host.start_resolver();
    let server = Server::lab(|incoming| {
        let body: Value = serde_json::from_slice(&incoming.body).unwrap();
        let pack = super::lab::pack_in(&body).unwrap();
        if pack["predecessor"]["availability"] == "committed" {
            let expiry = pack["references"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["owner"] == "cyber_terrain")
                .filter_map(|r| r["valid_until"].as_u64())
                .min()
                .unwrap();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap();
            let left = Duration::from_secs(expiry).saturating_sub(now) + Duration::from_millis(100);
            assert!(
                left < Duration::from_secs(31),
                "original DNS TTL is retained"
            );
            std::thread::sleep(left);
        }
        Reply::Length(
            200,
            serde_json::to_vec(&envelope(&super::lab::proposal(&pack))).unwrap(),
        )
    });
    let request = super::lab::registered(&mut host, "seed.duskweave.test");
    let (output, receipt) = super::lab::run(&mut host, &request);
    assert!(
        output.status.success(),
        "TTL-only expiry after release retains the proposal: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let decision = &receipt["recorded_decisions"].as_array().unwrap()[1];
    assert_eq!(decision["version"], 2);
    assert_eq!(decision["disposition"], "recorded_stale_premises");
    assert!(decision["proposal"].is_object());
    assert_eq!(receipt["decision_history_count"], 2);
    let run = request["run_id"].as_str().unwrap();
    for _ in 0..2 {
        let recovered = host.cli(&["m1-run-recover", "--run", run]);
        assert!(recovered.status.success());
        let view = Host::receipt(&recovered);
        assert_eq!(view["recorded_decisions"][1], *decision);
        assert_eq!(view["effect_count"], 1);
        assert_eq!(host.count("dns_udp"), 1);
        assert_eq!(server.count(), 2);
    }
    std::fs::write(
        host.report.join("stale-receipt.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "requires real READY, delayed authority response, private namespaces and owned local traps"]
fn delayed_current_check_cannot_start_or_renew_and_private_egress_probes_remain_closed() {
    let mut host = Host::new();
    host.start_resolver();
    let normal = std::fs::read(&host.environment["DW_INFERENCE_BROKER_DATABASE_URL_FILE"]).unwrap();
    let private = host.private_bytes(&normal);
    host.environment.insert(
        "DW_INFERENCE_BROKER_DATABASE_URL_FILE".into(),
        private.to_str().unwrap().into(),
    );
    let _restore = RestoreFile {
        path: private.clone(),
        bytes: normal.clone(),
    };
    let relay = wait_proxy::Relay::start(
        "127.0.0.1:5432".parse().unwrap(),
        b"execution.live_effect_current",
    );
    let source = String::from_utf8(normal.clone()).unwrap();
    let routed = source.replace("@127.0.0.1:5432/", &format!("@127.0.0.1:{}/", relay.port()));
    let route = private.clone();
    let server = Server::lab(move |incoming| {
        let pack =
            super::lab::pack_in(&serde_json::from_slice::<Value>(&incoming.body).unwrap()).unwrap();
        std::fs::write(&route, routed.as_bytes()).unwrap();
        Reply::Length(
            200,
            serde_json::to_vec(&envelope(&super::lab::proposal(&pack))).unwrap(),
        )
    });
    let request = super::lab::registered(&mut host, "seed.duskweave.test");
    let run: Uuid = serde_json::from_value(request["run_id"].clone()).unwrap();
    let file = host.write(&request);
    let mut parent = host.spawn(&[
        "m1-run",
        "--input",
        file.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-v1",
    ]);
    assert!(
        relay.observed(Duration::from_secs(8)),
        "normal completed current-authority response is delayed"
    );
    let attempt: Uuid = host
        .admin
        .query_one(
            "SELECT attempt FROM execution.autonomous_runs WHERE run_id=$1",
            &[&run],
        )
        .unwrap()
        .get(0);
    let journal =
        std::path::Path::new("/run/duskweave/launcher").join(attempt.simple().to_string());
    assert!(
        !journal.join("started").exists(),
        "no START before the current check completes"
    );
    for exporter in [false, true] {
        let probe = host.probe_namespace(attempt, exporter);
        assert!(
            probe.as_object().unwrap().values().all(|v| v == false),
            "resolver before START, wrong endpoint/protocol, metadata and IPv6 remain blocked"
        );
    }
    assert_eq!(host.count("dns_udp"), 0);
    assert_eq!(host.count("wrong_udp_port"), 0);
    assert_eq!(host.count("dns_tcp"), 0);
    std::thread::sleep(Duration::from_millis(1050));
    drop(relay);
    std::fs::write(&private, &normal).unwrap();
    assert!(
        parent
            .wait_until(Instant::now() + Duration::from_secs(4))
            .is_some()
    );
    let output = parent.output();
    assert!(
        !output.status.success(),
        "late authority reply cannot revive original READY/lease"
    );
    assert_eq!(host.count("dns_udp"), 0);
    assert_eq!(server.count(), 1);
    assert!(!journal.join("started").exists());
}

#[test]
#[ignore = "requires owned acquisition/core/watchdog process loss while one LAB query is in flight"]
fn owned_core_launcher_and_watchdog_loss_stop_workers_without_replay_or_refund() {
    for component in ["core", "launcher", "watchdog"] {
        let mut host = Host::new();
        host.start_resolver();
        let server = super::lab::server();
        let request = super::lab::registered(&mut host, "timeout.duskweave.test");
        let run: Uuid = serde_json::from_value(request["run_id"].clone()).unwrap();
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
                    "parent ended before the owned fault: {}",
                    String::from_utf8_lossy(&parent.output().stdout)
                );
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(host.count("dns_udp"), 1);
        let attempt: Uuid = host
            .admin
            .query_one(
                "SELECT attempt FROM execution.autonomous_runs WHERE run_id=$1",
                &[&run],
            )
            .unwrap()
            .get(0);
        let stopped = Instant::now();
        host.crash_owned_process(run, component);
        while !host.workers_absent(attempt) && stopped.elapsed() < Duration::from_millis(1000) {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            host.workers_absent(attempt),
            "owned workers must be gone after {component} loss"
        );
        assert!(
            stopped.elapsed() < Duration::from_millis(1000),
            "original post-loss disposal bound"
        );
        assert!(
            parent
                .wait_until(Instant::now() + Duration::from_secs(4))
                .is_some()
        );
        let terminal = parent.output();
        assert!(!terminal.status.success());
        let charged: bool = host
            .admin
            .query_one(
                "SELECT possibly_started FROM execution.live_effect_control WHERE run_id=$1",
                &[&run],
            )
            .unwrap()
            .get(0);
        assert!(charged);
        for _ in 0..2 {
            let recovered = host.cli(&["m1-run-recover", "--run", &run.to_string()]);
            assert!(recovered.status.success());
            assert_eq!(host.count("dns_udp"), 1);
            assert_eq!(server.count(), 1);
        }
    }
}
#[test]
#[ignore = "requires real committed/disposed result followed by an unavailable Terrain consumer"]
fn consumer_connection_loss_recovers_the_committed_result_without_another_acquisition() {
    let mut host = Host::new();
    host.start_resolver();
    let server = super::lab::server();
    let normal = std::fs::read(&host.environment["DW_TERRAIN_DATABASE_URL_FILE"]).unwrap();
    let source = String::from_utf8(normal.clone()).unwrap();
    let private = host.private_bytes(
        source
            .replace("@127.0.0.1:5432/", "@127.0.0.1:1/")
            .as_bytes(),
    );
    host.environment.insert(
        "DW_TERRAIN_DATABASE_URL_FILE".into(),
        private.to_str().unwrap().into(),
    );
    let _restore = RestoreFile {
        path: private.clone(),
        bytes: normal.clone(),
    };
    let request = super::lab::registered(&mut host, "seed.duskweave.test");
    let run: Uuid = serde_json::from_value(request["run_id"].clone()).unwrap();
    let (output, receipt) = super::lab::run(&mut host, &request);
    assert!(!output.status.success());
    assert_eq!(receipt["effect_count"], 1);
    assert_eq!(receipt["terrain_history_count"], 0);
    let before = result_witness(&mut host, run);
    assert_eq!(before["phase"], "closed");
    assert_eq!(before["disposal_confirmed"], true);
    std::fs::write(
        host.report.join("before-recovery.json"),
        serde_json::to_vec_pretty(&before).unwrap(),
    )
    .unwrap();
    std::fs::write(&private, &normal).unwrap();
    for _ in 0..2 {
        let output = host.cli(&["m1-run-recover", "--run", &run.to_string()]);
        assert!(output.status.success());
        let view = Host::receipt(&output);
        assert_eq!(view["terrain_history_count"], 1);
        let after = result_witness(&mut host, run);
        assert_eq!(after["result"], before["result"]);
        assert_eq!(after["projection"], before["projection"]);
        assert_eq!(host.count("dns_udp"), 1);
        assert_eq!(server.count(), 1);
    }
}
