use duskweave::m1_reasoning::{inference::*, inference_intent, request};
use duskweave::mission::{CampaignId, EngagementId};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

const LIVE: &str = "autonomous-dns-genai-live-v1";
const LAB: &str = "autonomous-dns-genai-lab-v1";

fn registration(run: &str, profile: &str, recipient: &str) -> Value {
    let mut r = super::consent::registration(
        EngagementId(Uuid::from_u128(0x51)),
        CampaignId(Uuid::from_u128(0x52)),
    );
    super::sdk_fixture::configure_permission(&mut r);
    let p = &mut r["m1_inference_permission"];
    p["version"] = json!("m1_inference_permission_v6");
    p["run_profile"] = json!(run);
    p["execution_mode"] = json!("live");
    p["profile"] = json!(profile);
    p["recipient"] = json!(recipient);
    p["live_binding"] = binding(if run == LAB { "192.0.2.53" } else { "8.8.8.8" });
    r
}
pub fn binding(resolver: &str) -> Value {
    json!({"qualification_ref":"00000000-0000-0000-0000-000000000071",
        "release_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "resolver_ipv4":resolver})
}

#[test]
fn v6_consent_keeps_live_and_lab_bindings_distinct() {
    for (run, profile, recipient) in [
        (
            LIVE,
            "deepseek-genai-responses-low-v1",
            "https://api.deepseek.com/responses",
        ),
        (
            LAB,
            "fixture-genai-responses-v1",
            "http://127.0.0.1:18083/responses",
        ),
    ] {
        let r = registration(run, profile, recipient);
        assert!(
            duskweave::input::parse_register(&serde_json::to_vec(&r).unwrap()).is_ok(),
            "fresh v6 registration must admit its closed binding"
        );
        for (key, wrong) in [
            ("version", "m1_inference_permission_v5"),
            ("execution_mode", "controlled_fixture"),
            ("recipient", "http://127.0.0.1:9999/responses"),
            ("run_profile", if run == LIVE { LAB } else { LIVE }),
        ] {
            let mut changed = r.clone();
            changed["m1_inference_permission"][key] = json!(wrong);
            assert!(
                duskweave::input::parse_register(&serde_json::to_vec(&changed).unwrap()).is_err(),
                "{key}"
            );
        }
        let mut array_binding = r.clone();
        let b = &r["m1_inference_permission"]["live_binding"];
        array_binding["m1_inference_permission"]["live_binding"] = json!([
            b["qualification_ref"],
            b["release_sha256"],
            b["resolver_ipv4"]
        ]);
        assert!(
            duskweave::input::parse_register(&serde_json::to_vec(&array_binding).unwrap()).is_err(),
            "live binding must be an object"
        );
    }
}

#[test]
fn v4_run_is_explicit_and_legacy_versions_cannot_authorize_live() {
    let mut q = live_run_request();
    assert!(
        duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&q).unwrap()).is_ok(),
        "live v4 request must survive syntax admission; qualification grants release separately"
    );
    for version in [
        "m1_autonomous_run_request_v1",
        "m1_autonomous_run_request_v2",
        "m1_autonomous_run_request_v3",
    ] {
        q["version"] = json!(version);
        assert!(duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&q).unwrap()).is_err());
    }
}

fn live_run_request() -> Value {
    let mut q = super::sdk_fixture::run_request();
    q["version"] = json!("m1_autonomous_run_request_v4");
    q["run_profile"] = json!(LAB);
    q["execution_mode"] = json!("live");
    q["live_binding"] = binding("192.0.2.53");
    q["seed_names"][0]["name"] = json!("seed.duskweave.test");
    q
}

#[cfg(windows)]
#[test]
fn operational_cli_refuses_live_on_windows_before_database_or_provider() {
    use std::io::Write;
    let path = std::env::temp_dir().join(format!(
        "dw-live-admission-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.write_all(&serde_json::to_vec(&live_run_request()).unwrap())
        .unwrap();
    drop(file);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_duskweave"))
        .args(["m1-run", "--input"])
        .arg(&path)
        .args(["--timing-profile", "reasoning-bounded-v1"])
        .env_clear()
        .env("SystemRoot", std::env::var_os("SystemRoot").unwrap())
        .output()
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(!output.status.success());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains("live_host_platform_unqualified"),
        "platform refusal must precede missing DSNs: {text}"
    );
    assert!(!text.contains("missing_env"));
}

#[test]
fn v4_pack_uses_semantic_intent_and_never_legacy_wire_binding() {
    let s = Scope {
        engagement: Uuid::from_u128(1),
        campaign: Uuid::from_u128(2),
        decision: Uuid::from_u128(3),
        permission: Uuid::from_u128(4),
        mission_revision: 1,
    };
    let p = serde_json::to_vec(&json!({"version":"m1_context_pack_autonomous_v4",
        "run_profile":LAB,"execution_mode":"live","inference_profile":"fixture-genai-responses-v1","live_binding":binding("192.0.2.53"),"qualification_purpose":"LAB",
        "engagement_id":s.engagement,"campaign_id":s.campaign,"decision_id":s.decision,
        "permission_id":s.permission,"mission_revision":1,"references":[{"id":"asset_0"}],
        "manifests":[{"reference":"dns_udp"}]}))
    .unwrap();
    assert!(
        inference_intent::sdk_pack(&p),
        "v4 must enter SDK semantic dispatch"
    );
    assert!(
        request::bind(&p, None).is_err(),
        "legacy transport cannot bind v4"
    );
    let r = Reservation {
        generation: 1,
        generation_call: Uuid::from_u128(5),
        repair_call: Uuid::from_u128(6),
        shared_deadline: Instant::now() + Duration::from_secs(124),
    };
    let bound = inference_intent::bind(&s, &r, r.generation_call, &p, None).unwrap();
    assert!(bound.body.is_empty());
    let intent = inference_intent::verify(&bound).unwrap();
    assert_eq!(intent["permission_version"], "m1_inference_permission_v6");
    assert_eq!(intent["run_version"], "m1_autonomous_run_request_v4");
    assert_eq!(intent["run_profile"], LAB);
    assert_eq!(intent["execution_mode"], "live");
    assert_eq!(intent["live_binding"], binding("192.0.2.53"));
}
