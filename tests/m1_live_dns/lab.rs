//! Normal installed CLI LAB episodes; SDK fixture output is explicitly synthetic.
#![cfg(target_os = "linux")]
use super::host::Host;
use super::sdk_fixture::{Reply, Server, envelope};
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) fn pack_in(value: &Value) -> Option<Value> {
    match value {
        Value::String(s) => serde_json::from_str::<Value>(s)
            .ok()
            .filter(|p| p["version"] == "m1_context_pack_autonomous_v4"),
        Value::Array(a) => a.iter().find_map(pack_in),
        Value::Object(o) => o.values().find_map(pack_in),
        _ => None,
    }
}
pub(super) fn proposal(pack: &Value) -> Value {
    let feedback = pack["predecessor"]["availability"] == "committed";
    let (refs, intent) = if feedback {
        let reference = pack["references"].as_array().unwrap().iter().find(|r| {
            r["owner"] == "cyber_terrain"
                && r["source"] == "issued_dns_acquisition"
                && r["status"] == "observed"
        });
        match reference {
            Some(reference) => {
                let id = reference["id"].clone();
                (
                    json!(["previous_decision", id]),
                    json!({"category":"retain","references":[id],"manifest_reference":null}),
                )
            }
            None => (
                json!(["previous_decision", "mission"]),
                json!({"category":"defer","references":["mission"],"manifest_reference":null}),
            ),
        }
    } else {
        (
            json!(["asset_0"]),
            json!({"category":"observe_more","references":["mission","asset_0"],
            "manifest_reference":"dns_udp","view_request":null}),
        )
    };
    json!({"version":"m1_inference_proposal_v1","hypothesis":"The admitted routing premise merits one bounded observation.",
        "considered_alternative":"Routing may remain unavailable.","references":refs,
        "expected_observation":"Qualified routing feedback.","falsifying_observation":"Missing or conflicting owner feedback.",
        "uncertainty":"Synthetic LAB plumbing only; live applicability is unknown.",
        "rationale":"Retain the admitted observation and its limitations.","next_intent":intent})
}
pub(super) fn registered(host: &mut Host, name: &str) -> Value {
    use duskweave::mission::{CampaignId, EngagementId};
    let (e, c, run) = (host.id(), host.id(), host.id());
    let mut registration = super::consent::registration(EngagementId(e), CampaignId(c));
    super::sdk_fixture::configure_permission(&mut registration);
    let now: i64 = host
        .admin
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    registration["starts_at"] = json!(now - 1);
    registration["ends_at"] = json!(now + 3600);
    registration["m1_permission"] = json!({
        "policy_version":1,"ct_base_domain":"duskweave.test","provider_disclosure":"crt_sh",
        "vantage_ref":host.id(),"resolver_ipv4":"192.0.2.53",
        "discovery_rules":[{"label_suffix":"duskweave.test"}],
        "contact_rules":[{"asset_ref":registration["included_assets"][0],"rule":{"exact":name},"priority":1}],
        "excluded_names":[],"approved_path":"/","starts_at":now,"ends_at":now+3599,
        "campaign_limits":{"episodes":1,"provider_calls":1,"dns_questions":1,"dns_followups":1,
            "tcp_connections":1,"head_requests":1},"concurrency":1
    });
    let p = &mut registration["m1_inference_permission"];
    p["version"] = json!("m1_inference_permission_v6");
    p["execution_mode"] = json!("live");
    p["run_profile"] = json!("autonomous-dns-genai-lab-v1");
    p["live_binding"] = host.live_binding();
    p["starts_at"] = json!(now);
    p["ends_at"] = json!(now + 3599);
    p["permission_id"] = json!(host.id());
    let file = host.write(&registration);
    let operation = host.id();
    let output = host.cli(&[
        "register",
        "--operation",
        &operation.to_string(),
        "--input",
        file.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "restricted normal registration: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let mut request = super::sdk_fixture::run_request();
    request["version"] = json!("m1_autonomous_run_request_v4");
    request["run_id"] = json!(run);
    request["run_profile"] = json!("autonomous-dns-genai-lab-v1");
    request["execution_mode"] = json!("live");
    request["live_binding"] = host.live_binding();
    request["deadline_at_ms"] = json!(now * 1000 + 400000);
    let mission = &mut request["context"]["mission"];
    mission["engagement_id"] = json!(e);
    mission["campaign_id"] = json!(c);
    mission["operator_ref"] = registration["operator_ref"].clone();
    mission["permission_id"] = registration["m1_inference_permission"]["permission_id"].clone();
    mission["goal_ref"] = registration["goal_ref"].clone();
    request["context"]["candidates"][0]["asset_ref"] = registration["included_assets"][0].clone();
    request["seed_names"][0]["asset_ref"] = registration["included_assets"][0].clone();
    request["seed_names"][0]["name"] = json!(name);
    request
}
pub(super) fn server() -> Server {
    server_with("observe_more")
}
pub(super) fn server_with(action: &'static str) -> Server {
    Server::lab(move |incoming| {
        let body: Value = serde_json::from_slice(&incoming.body).unwrap();
        let pack = pack_in(&body).expect("actual v4 SDK request pack");
        let mut candidate = proposal(&pack);
        if pack["predecessor"]["availability"] != "committed" && action != "observe_more" {
            candidate["references"] = json!(["mission"]);
            candidate["next_intent"] =
                json!({"category":action,"references":["mission"],"manifest_reference":null});
        }
        Reply::Length(200, serde_json::to_vec(&envelope(&candidate)).unwrap())
    })
}
pub(super) fn run(host: &mut Host, request: &Value) -> (std::process::Output, Value) {
    let file = host.write(request);
    let output = host.cli(&[
        "m1-run",
        "--input",
        file.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-v1",
    ]);
    let receipt = Host::receipt(&output);
    (output, receipt)
}
#[test]
#[ignore = "requires the owned Oracle installed release, fresh schema-0030 b, isolated resolver and root"]
fn normal_cli_lab_reaches_one_dns_two_decisions_and_key_free_recovery() {
    let mut host = Host::new();
    host.start_resolver();
    let server = server();
    let request = registered(&mut host, "seed.duskweave.test");
    let (output, receipt) = run(&mut host, &request);
    std::fs::write(
        host.report.join("receipt.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
    assert!(
        output.status.success(),
        "normal CLI: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(receipt["effect_count"], 1, "real committed live effect");
    assert_eq!(receipt["decision_count"], 2);
    assert_eq!(receipt["decision_history_count"], 2);
    assert_eq!(server.count(), 2, "one generation for each decision");
    assert_eq!(
        host.count("dns_udp"),
        1,
        "exactly one real admitted UDP question"
    );
    assert_eq!(host.count("wrong_udp_port"), 0);
    assert_eq!(host.count("dns_tcp"), 0);
    let run: Uuid = serde_json::from_value(request["run_id"].clone()).unwrap();
    let row=host.admin.query_one("SELECT x.possibly_started,x.disposal_confirmed,x.phase,s.result_ref FROM execution.live_effect_sessions s JOIN execution.live_effect_control x USING(run_id) WHERE s.run_id=$1",&[&run]).unwrap();
    assert!(row.get::<_, bool>(0) && row.get::<_, bool>(1));
    assert_eq!(row.get::<_, String>(2), "closed");
    let result: Uuid = row.get(3);
    let requests = server.requests.lock().unwrap();
    let d2 = pack_in(&serde_json::from_slice(&requests[1].body).unwrap()).unwrap();
    assert!(
        d2.to_string().contains(&result.to_string()),
        "D2 receives immutable issued lineage"
    );
    drop(requests);
    for _ in 0..2 {
        let recovered = host.cli(&["m1-run-recover", "--run", &run.to_string()]);
        assert!(recovered.status.success(), "fresh key-free recovery");
        let view = Host::receipt(&recovered);
        assert_eq!(view["decision_count"], 2);
        assert_eq!(view["effect_count"], 1);
        assert_eq!(host.count("dns_udp"), 1);
        assert_eq!(server.count(), 2);
    }
}

#[test]
#[ignore = "requires the owned installed release and resolver counters; no DNS authority selected"]
fn d1_defer_and_stop_terminate_without_acquisition_or_second_generation() {
    for (intent, status) in [("defer", "deferred"), ("stop", "stopped")] {
        let mut host = Host::new();
        host.start_resolver();
        let server = server_with(intent);
        let request = registered(&mut host, "seed.duskweave.test");
        let (output, receipt) = run(&mut host, &request);
        assert!(output.status.success(), "normal D1 terminal outcome");
        assert_eq!(receipt["status"], status);
        assert_eq!(receipt["decision_count"], 1);
        assert_eq!(receipt["decision_history_count"], 1);
        assert_eq!(receipt["effect_count"], 0);
        assert_eq!(receipt["acquisition"], "not_reserved");
        assert_eq!(host.count("dns_udp"), 0);
        assert_eq!(server.count(), 1);
    }
}
#[test]
#[ignore = "requires the owned real resolver scenarios; limited outcomes cannot become positive routing"]
fn real_negative_limited_and_ttl_zero_results_preserve_categories_and_no_routing_claim() {
    for (name, status, rcode, truncated) in [
        ("nxdomain", "completed", 3, false),
        ("empty", "completed", 0, false),
        ("truncated", "limited", 0, true),
        ("malformed", "unsupported_response", 0, false),
        ("sentinel", "unsupported_response", 0, false),
        ("ttl-zero", "completed", 0, false),
    ] {
        let mut host = Host::new();
        host.start_resolver();
        let server = server();
        let request = registered(&mut host, &format!("{name}.duskweave.test"));
        let (_output, receipt) = run(&mut host, &request);
        let run: Uuid = serde_json::from_value(request["run_id"].clone()).unwrap();
        let row = host
            .admin
            .query_one(
                "SELECT projection FROM execution.live_dns_results WHERE run_id=$1",
                &[&run],
            )
            .expect("real limited result remains durably recorded");
        let projection: Value = row.get(0);
        assert_eq!(projection["status"], status, "scenario {name}");
        assert_eq!(projection["rcode"], rcode);
        assert_eq!(projection["truncated"], truncated);
        assert_eq!(receipt["effect_count"], 1);
        assert_eq!(host.count("dns_udp"), 1);
        assert_eq!(
            server.count(),
            2,
            "D2 receives the bounded outcome and defers"
        );
        assert_eq!(
            receipt["recorded_decisions"][1]["proposal"]["next_intent"]["category"],
            "defer"
        );
        assert!(
            !receipt
                .to_string()
                .contains("RAW_DNS_SENTINEL_MUST_NOT_ESCAPE")
        );
        let claim:String=host.admin.query_one("SELECT t.contract#>>'{claim,claim_status}' FROM terrain.outbox t JOIN execution.autonomous_runs x ON t.operation_id=x.terrain_operation WHERE x.run_id=$1",&[&run]).unwrap().get(0);
        assert_eq!(claim, "withheld");
        std::fs::write(
            host.report.join("limited-receipt.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
    }
}

#[test]
#[ignore = "requires the actual two-second unanswered UDP attempt and trusted terminal result"]
fn real_timeout_keeps_the_honest_result_and_allows_bounded_outcome_feedback() {
    let mut host = Host::new();
    host.start_resolver();
    let server = server();
    let request = registered(&mut host, "timeout.duskweave.test");
    let (output, receipt) = run(&mut host, &request);
    assert_eq!(
        receipt["effect_count"],
        1,
        "trusted timeout is a durable bounded outcome: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let run: Uuid = serde_json::from_value(request["run_id"].clone()).unwrap();
    let projection: Value = host
        .admin
        .query_one(
            "SELECT projection FROM execution.live_dns_results WHERE run_id=$1",
            &[&run],
        )
        .unwrap()
        .get(0);
    assert_eq!(projection["status"], "timeout");
    assert_eq!(projection["observed_at_ms"], 0);
    assert!(projection["answers"].as_array().unwrap().is_empty());
    assert!(output.status.success());
    assert_eq!(receipt["decision_count"], 2);
    assert_eq!(
        receipt["recorded_decisions"][1]["proposal"]["next_intent"]["category"],
        "defer"
    );
    assert_eq!(host.count("dns_udp"), 1);
    assert_eq!(server.count(), 2);
}

#[test]
#[ignore = "serial packet-owned LAB only; requires installed release and no active campaign"]
fn installed_release_rejects_writable_manifest_before_provider_or_dns() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    const MANIFEST: &str = "/etc/duskweave/m1/deployment.json";
    struct RestoreMode(u32);
    impl Drop for RestoreMode {
        fn drop(&mut self) {
            std::fs::set_permissions(MANIFEST, std::fs::Permissions::from_mode(self.0))
                .expect("restore exact packet-owned manifest mode");
        }
    }

    let mut host = Host::new();
    host.start_resolver();
    let server = server();
    let request = registered(&mut host, "seed.duskweave.test");
    let input = host.write(&request);
    let before = std::fs::symlink_metadata(MANIFEST).unwrap();
    let bytes = std::fs::read(MANIFEST).unwrap();
    assert_eq!(before.uid(), 0);
    assert_eq!(before.nlink(), 1);
    {
        let _restore = RestoreMode(before.mode() & 0o7777);
        std::fs::set_permissions(MANIFEST, std::fs::Permissions::from_mode(0o666)).unwrap();
        let output = host.cli(&[
            "m1-run",
            "--input",
            input.to_str().unwrap(),
            "--timing-profile",
            "reasoning-bounded-v1",
        ]);
        assert!(
            !output.status.success(),
            "writable manifest must refuse preflight"
        );
        assert_eq!(server.count(), 0, "no provider fixture contact");
        assert_eq!(host.count("dns_udp"), 0, "no DNS question");
    }
    let after = std::fs::symlink_metadata(MANIFEST).unwrap();
    assert_eq!(std::fs::read(MANIFEST).unwrap(), bytes);
    assert_eq!(
        (after.uid(), after.gid(), after.mode(), after.nlink()),
        (before.uid(), before.gid(), before.mode(), before.nlink())
    );
}
