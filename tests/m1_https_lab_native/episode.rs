#![allow(dead_code)]
//! Same-run `m1-run` -> D1 -> LAB DNS -> Terrain -> D2 -> LAB HEAD -> Terrain -> D3 -> inspect/recover witnesses.
#[cfg(target_os = "linux")]
use super::support::linux_host::NativeHost;
#[cfg(target_os = "linux")]
use super::support::{build_native_run_bundle, start_loopback_sdk, start_loopback_sdk_with_delay};
#[cfg(target_os = "linux")]
use crate::lab_support::sanitize_inspect_receipt;
#[cfg(target_os = "linux")]
use serde_json::Value;
#[cfg(target_os = "linux")]
use std::time::Duration;
#[cfg(target_os = "linux")]
use uuid::Uuid;

#[cfg(target_os = "linux")]
fn run_episode(host: &mut NativeHost, run_id: Uuid) -> (Value, std::process::Output) {
    let e = host.id();
    let c = host.id();
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
    let run_file = host.write_input(&req);
    let out = host.cli(&[
        "m1-run",
        "--input",
        run_file.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-pro-v2",
    ]);
    let rcpt = NativeHost::receipt(&out);
    (rcpt, out)
}

#[cfg(target_os = "linux")]
fn verify_inspect_and_recover(
    host: &mut NativeHost,
    label: &str,
    run_id: Uuid,
    run_receipt: &Value,
    exp_dns: u64,
    exp_head: u64,
) {
    let san_run = sanitize_inspect_receipt(run_receipt).unwrap();
    let insp = host.cli(&["m1-run-inspect", "--run", &run_id.to_string()]);
    assert!(
        insp.status.success(),
        "m1-run-inspect: {}",
        String::from_utf8_lossy(&insp.stderr)
    );
    let insp_val = NativeHost::receipt(&insp);
    let insp_san = sanitize_inspect_receipt(&insp_val).unwrap();
    assert_eq!(insp_san, san_run);

    let rec = host.cli(&["m1-run-recover", "--run", &run_id.to_string()]);
    assert!(
        rec.status.success(),
        "m1-run-recover: {}",
        String::from_utf8_lossy(&rec.stderr)
    );
    let rec_val = NativeHost::receipt(&rec);
    let rec_san = sanitize_inspect_receipt(&rec_val).unwrap();
    assert_eq!(rec_san, san_run);
    assert_eq!(host.dns_count("dns_answered"), exp_dns);
    assert_eq!(host.https_counters()["head_requests"], exp_head);
    host.save_inspect_artifact(
        label,
        run_id,
        &String::from_utf8_lossy(&insp.stdout),
        &insp_san,
        &rec_san,
    );
}

#[cfg(target_os = "linux")]
#[ignore = "requires reviewed install, schema-0038 DB, and isolated ARM64 netns topology"]
#[test]
fn native_same_run_dns_head_200_and_302_feedback_and_key_free_inspect() {
    for (scenario, exp_status, exp_redir) in
        [("200", 200i32, "none"), ("302", 302i32, "refused_redirect")]
    {
        let mut host = NativeHost::new();
        let run_id = host.id();
        host.start_topology_for(scenario, run_id, 30, 30);
        let sdk = start_loopback_sdk("none");
        let (receipt, out) = run_episode(&mut host, run_id);
        assert!(
            out.status.success(),
            "m1-run {scenario}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(receipt["status"], "retained");
        assert_eq!(receipt["live_qualified"], true);
        assert_eq!(receipt["decision_count"], 3);
        assert_eq!(receipt["decision_history_count"], 3);
        assert_eq!(receipt["dns_effect_count"], 1);
        assert_eq!(receipt["head_effect_count"], 1);
        assert_eq!(receipt["terrain_history_count"], 2);
        assert_eq!(receipt["steps"][1]["step_index"], 2);
        assert_eq!(receipt["steps"][1]["status"], "completed");
        assert_eq!(receipt["steps"][1]["head_outcome"], "head_response");
        assert_eq!(receipt["steps"][2]["step_index"], 3);
        assert_eq!(receipt["steps"][2]["status"], "terminal");
        assert_eq!(receipt["steps"][2]["disposition"], "retained");
        assert_eq!(receipt["recorded_decisions"][2]["step_index"], 3);
        assert_eq!(
            receipt["recorded_decisions"][2]["disposition"],
            "validated_proposal"
        );
        assert_eq!(
            receipt["recorded_decisions"][2]["proposal"]["next_intent"]["category"],
            "retain"
        );
        assert_eq!(sdk.count(), 3);

        let hc = host.https_counters();
        assert_eq!(hc["run_id"], run_id.to_string());
        assert_eq!(hc["dns_queries"], 1);
        assert_eq!(hc["dns_answered"], 1);
        assert_eq!(hc["dns_refused"], 0);
        assert_eq!(hc["connections"], 1);
        assert_eq!(hc["tls_handshakes"], 1);
        assert_eq!(hc["head_requests"], 1);

        let sql_row = host
            .admin
            .query_one(
                "SELECT s.status, s.head_outcome, a.release_sha256,
                    lr.result_payload->>'outcome', (lr.result_payload->>'status_code')::int,
                    lr.result_payload->>'redirect'
             FROM execution.https_loop_steps s
             JOIN execution.autonomous_runs a USING (run_id)
             JOIN execution.live_https_results lr
               ON (lr.run_id, lr.step_index, lr.attempt, lr.result)
                = (s.run_id, s.step_index, s.head_attempt_id, s.head_result_id)
             WHERE s.run_id = $1 AND s.step_index = 2",
                &[&run_id],
            )
            .unwrap();
        assert_eq!(sql_row.get::<_, String>(0), "completed");
        assert_eq!(sql_row.get::<_, String>(1), "head_response");
        assert_eq!(
            sql_row.get::<_, String>(2),
            host.manifest["release_sha256"].as_str().unwrap()
        );
        assert_eq!(sql_row.get::<_, String>(3), "head_response");
        assert_eq!(sql_row.get::<_, i32>(4), exp_status);
        assert_eq!(sql_row.get::<_, String>(5), exp_redir);

        verify_inspect_and_recover(
            &mut host,
            &format!("episode-{scenario}"),
            run_id,
            &receipt,
            1,
            1,
        );
    }
}

#[cfg(target_os = "linux")]
#[ignore = "requires reviewed install, schema-0038 DB, and isolated ARM64 netns topology"]
#[test]
fn native_conditional_refresh_on_expired_short_ttl_claim() {
    let mut host = NativeHost::new();
    let run_id = host.id();
    host.start_topology_for("200", run_id, 2, 30);
    let sdk = start_loopback_sdk_with_delay("dns_once_if_stale", Duration::from_millis(2300));
    let (receipt, out) = run_episode(&mut host, run_id);
    assert!(
        out.status.success(),
        "m1-run refresh: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(receipt["status"], "retained");
    assert_eq!(receipt["live_qualified"], true);
    assert_eq!(receipt["decision_count"], 3);
    assert_eq!(receipt["decision_history_count"], 3);
    assert_eq!(receipt["dns_effect_count"], 2);
    assert_eq!(receipt["head_effect_count"], 1);
    assert_eq!(receipt["terrain_history_count"], 3);
    assert_eq!(sdk.count(), 3);
    let hc = host.https_counters();
    assert_eq!(hc["run_id"], run_id.to_string());
    assert_eq!(host.dns_count("dns_answered"), 2);
    assert_eq!(host.dns_count("dns_refused"), 0);
    assert_eq!(hc["head_requests"], 1);

    let row = host
        .admin
        .query_one(
            "SELECT s.refresh_attempt_id IS NOT NULL, s.refresh_result_id IS NOT NULL,
                    s.refresh_outcome, s.resolved_endpoint_ref, dr.attempt_kind,
                    lr.result_payload->>'endpoint_ref',
                    (lr.result_payload->>'parent_claim_revision')::int
             FROM execution.https_loop_steps s
             JOIN execution.live_dns_results dr
               ON (dr.run_id, dr.step_index, dr.attempt, dr.result)
                = (s.run_id, s.step_index, s.refresh_attempt_id, s.refresh_result_id)
             JOIN execution.live_https_results lr
               ON (lr.run_id, lr.step_index, lr.attempt, lr.result)
                = (s.run_id, s.step_index, s.head_attempt_id, s.head_result_id)
             WHERE s.run_id = $1 AND s.step_index = 2",
            &[&run_id],
        )
        .unwrap();
    assert!(row.get::<_, bool>(0) && row.get::<_, bool>(1));
    assert_eq!(row.get::<_, String>(2), "completed");
    assert_eq!(row.get::<_, String>(3), "endpoint_0");
    assert_eq!(row.get::<_, String>(4), "dns_refresh");
    assert_eq!(row.get::<_, String>(5), "endpoint_0");
    assert_eq!(row.get::<_, i32>(6), 2);

    verify_inspect_and_recover(&mut host, "episode-refresh", run_id, &receipt, 2, 1);
}
