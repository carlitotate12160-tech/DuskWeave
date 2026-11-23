#![allow(dead_code)]
//! Same-run `m1-run` -> D1 -> LAB DNS -> Terrain -> D2 -> LAB HEAD -> Terrain -> D3 -> inspect/recover witnesses.
#[cfg(target_os = "linux")]
use super::support::linux_host::NativeHost;
#[cfg(target_os = "linux")]
use super::support::{build_native_run_bundle, start_loopback_sdk};
#[cfg(target_os = "linux")]
use crate::lab_support::sanitize_inspect_receipt;
#[cfg(target_os = "linux")]
use serde_json::{Value, json};
#[cfg(target_os = "linux")]
use uuid::Uuid;

#[cfg(target_os = "linux")]
fn run_episode(host: &mut NativeHost, dep_refresh: &str) -> (Uuid, Value, std::process::Output) {
    let e = host.id();
    let c = host.id();
    let run_id = host.id();
    let now_s = host.db_now_s();
    let (reg, req) =
        build_native_run_bundle(e, c, run_id, &host.manifest, now_s, now_s * 1000 + 400_000);
    let reg_file = host.write_input(&reg);
    let op = host.id();
    let reg_out = host.cli(&[
        "register",
        "--operation",
        &op.to_string(),
        "--input",
        reg_file.to_str().unwrap(),
    ]);
    assert!(
        reg_out.status.success(),
        "register: {}",
        String::from_utf8_lossy(&reg_out.stderr)
    );
    let mut run_req = req;
    if dep_refresh == "dns_once_if_stale" {
        run_req["step_two_refresh_hint"] = json!("dns_once_if_stale");
    }
    let run_file = host.write_input(&run_req);
    let out = host.cli(&[
        "m1-run",
        "--input",
        run_file.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-pro-v2",
    ]);
    let rcpt = NativeHost::receipt(&out);
    (run_id, rcpt, out)
}

#[cfg(target_os = "linux")]
#[ignore = "requires reviewed install, schema-0038 DB, and isolated ARM64 netns topology"]
#[test]
fn native_same_run_dns_head_200_and_302_feedback_and_key_free_inspect() {
    for (scenario, exp_redir) in [("200", "none"), ("302", "refused_redirect")] {
        let mut host = NativeHost::new();
        host.start_topology(scenario);
        let sdk = start_loopback_sdk("none");
        let (run_id, receipt, out) = run_episode(&mut host, "none");
        assert!(
            out.status.success(),
            "m1-run {scenario}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(receipt["status"], "retained");
        assert_eq!(receipt["live_qualified"], true);
        assert_eq!(receipt["decision_count"], 3);
        assert_eq!(receipt["dns_effect_count"], 1);
        assert_eq!(receipt["head_effect_count"], 1);
        assert_eq!(sdk.count(), 3);
        assert_eq!(host.dns_count("dns_udp"), 1);
        assert_eq!(host.https_counters()["connections"], 1);
        assert_eq!(host.https_counters()["tls_handshakes"], 1);
        assert_eq!(host.https_counters()["head_requests"], 1);

        let sql_row = host
            .admin
            .query_one(
                "SELECT s.status, s.head_outcome, s.head_result_id, a.release_sha256,
                    hr.redirect_disposition
             FROM execution.https_loop_steps s
             JOIN execution.autonomous_runs a USING (run_id)
             JOIN execution.https_loop_head_results hr ON hr.result_id = s.head_result_id
             WHERE s.run_id = $1 AND s.step_index = 2",
                &[&run_id],
            )
            .unwrap();
        assert_eq!(sql_row.get::<_, String>(0), "completed");
        assert_eq!(sql_row.get::<_, String>(1), "head_response");
        assert_eq!(
            sql_row.get::<_, String>(3),
            host.manifest["release_sha256"].as_str().unwrap()
        );
        assert_eq!(sql_row.get::<_, String>(4), exp_redir);

        let san_run = sanitize_inspect_receipt(&receipt).unwrap();
        for cmd_name in ["m1-run-inspect", "m1-run-recover"] {
            let co = host.cli(&[cmd_name, "--run", &run_id.to_string()]);
            assert!(
                co.status.success(),
                "{cmd_name}: {}",
                String::from_utf8_lossy(&co.stderr)
            );
            let v = NativeHost::receipt(&co);
            assert_eq!(sanitize_inspect_receipt(&v).unwrap(), san_run);
            assert_eq!(host.dns_count("dns_udp"), 1);
            assert_eq!(host.https_counters()["head_requests"], 1);
        }
    }
}

#[cfg(target_os = "linux")]
#[ignore = "requires reviewed install, schema-0038 DB, and isolated ARM64 netns topology"]
#[test]
fn native_conditional_refresh_on_expired_short_ttl_claim() {
    let mut host = NativeHost::new();
    host.start_topology("200");
    let sdk = start_loopback_sdk("dns_once_if_stale");
    let (run_id, receipt, out) = run_episode(&mut host, "dns_once_if_stale");
    assert!(
        out.status.success(),
        "m1-run refresh: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(receipt["live_qualified"], true);
    assert_eq!(sdk.count(), 3);
    assert!(host.dns_count("dns_udp") >= 1);
    assert_eq!(host.https_counters()["head_requests"], 1);
    let _ = host
        .admin
        .query_one(
            "SELECT count(*) FROM execution.https_loop_steps WHERE run_id=$1",
            &[&run_id],
        )
        .unwrap();
}
