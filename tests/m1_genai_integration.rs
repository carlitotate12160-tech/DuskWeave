//! SDK successor ingress and real CLI/PostgreSQL integration.
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_genai_fixture.rs"]
mod fixture;

#[test]
fn v3_run_request_admits_only_explicit_sdk_profile() {
    let raw = serde_json::to_vec(&fixture::run_request()).unwrap();
    assert!(
        duskweave::m1_run::RunRequest::parse(&raw).is_ok(),
        "new versioned SDK run must be admitted"
    );
    for version in [
        "m1_autonomous_run_request_v1",
        "m1_autonomous_run_request_v2",
    ] {
        let mut q = fixture::run_request();
        q["version"] = serde_json::json!(version);
        assert!(duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&q).unwrap()).is_err());
    }
    let mut q = fixture::run_request();
    q["inference_profile"] = serde_json::json!("fixture-responses-v1");
    assert!(duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&q).unwrap()).is_err());
}

#[test]
fn v5_registration_admits_explicit_sdk_permission() {
    let mut value = consent::registration(
        duskweave::mission::EngagementId(uuid::Uuid::from_u128(0x21)),
        duskweave::mission::CampaignId(uuid::Uuid::from_u128(0x22)),
    );
    fixture::configure_permission(&mut value);
    let raw = serde_json::to_vec(&value).unwrap();
    assert!(
        duskweave::input::parse_register(&raw).is_ok(),
        "new versioned SDK permission must be admitted"
    );
    for version in [
        "m1_inference_permission_v1",
        "m1_inference_permission_v2",
        "m1_inference_permission_v3",
        "m1_inference_permission_v4",
    ] {
        value["m1_inference_permission"]["version"] = serde_json::json!(version);
        assert!(duskweave::input::parse_register(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}

#[path = "support/m1_terrain_context.rs"]
mod terrain;
use terrain::terrain_support::native;
#[path = "support/m1_run.rs"]
mod run_support;

fn refresh() {
    run_support::refresh();
    native::admin()
        .batch_execute(include_str!(
            "../migrations/0022_genai_inference_intent.sql"
        ))
        .unwrap();
}
fn registered(slot: u128) -> serde_json::Value {
    registered_with(slot, |_| {})
}
fn registered_with(
    slot: u128,
    configure: impl FnOnce(&mut serde_json::Value),
) -> serde_json::Value {
    let mut q = run_support::registered_with(slot, |raw| {
        fixture::configure_permission(raw);
        configure(raw);
    });
    q["version"] = serde_json::json!("m1_autonomous_run_request_v3");
    q["inference_profile"] = serde_json::json!(fixture::FIXTURE_PROFILE);
    q["deadline_at_ms"] = serde_json::json!(q["deadline_at_ms"].as_i64().unwrap() + 250000);
    q
}
#[path = "support/m1_inference_lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;

#[test]
fn sdk_schema0022_installs_private_semantic_authority_on_fresh_or_upgraded_fixture() {
    let _guard = native::db();
    let mut admin = native::admin();
    admin
        .batch_execute(include_str!(
            "../migrations/0022_genai_inference_intent.sql"
        ))
        .unwrap();
    let count:i64=admin.query_one("SELECT count(*) FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname='execution' AND p.proname LIKE 'genai_%'",&[]).unwrap().get(0);
    assert_eq!(count, 5);
    let public:i64=admin.query_one("SELECT count(*) FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace CROSS JOIN LATERAL aclexplode(coalesce(p.proacl,acldefault('f',p.proowner))) a WHERE n.nspname='execution' AND p.proname LIKE 'genai_%' AND a.grantee=0 AND a.privilege_type='EXECUTE'",&[]).unwrap().get(0);
    assert_eq!(public, 0, "new authority helpers must revoke PUBLIC");
    let error = match native::client("DW_TEST_DATABASE_URL").query_one(
        "SELECT execution.genai_binding_digest('pack','intent')",
        &[],
    ) {
        Err(error) => error,
        Ok(_) => panic!("restricted core cannot invoke private SDK authority"),
    };
    assert_eq!(error.code().unwrap().code(), "42501");
}

#[test]
fn sdk_deepseek_missing_or_invalid_key_never_falls_back_or_sends() {
    let _guard = native::db();
    refresh();
    let server = fixture::Server::start("127.0.0.1:18083", |_| fixture::Reply::Reset);
    for (index, key) in [None, Some("synthetic\ninvalid")].into_iter().enumerate() {
        let mut q = registered_with(0x2a500 + index as u128, |raw| {
            raw["m1_inference_permission"]["profile"] =
                serde_json::json!("deepseek-genai-responses-low-v1");
            raw["m1_inference_permission"]["recipient"] =
                serde_json::json!("https://api.deepseek.com/responses");
        });
        q["inference_profile"] = serde_json::json!("deepseek-genai-responses-low-v1");
        let path = run_support::path(&q);
        let mut command = run_support::command();
        command
            .env("DW_M1_RUN_CONTROL_ROOT", run_support::control_root())
            .env_remove("DEEPSEEK_API_KEY");
        if let Some(key) = key {
            command.env("DEEPSEEK_API_KEY", key);
        }
        let output = command
            .args([
                "m1-run",
                "--input",
                path.to_str().unwrap(),
                "--timing-profile",
                "reasoning-bounded-v1",
            ])
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&output.stdout);
        let r: serde_json::Value =
            serde_json::from_str(text.lines().find(|l| l.starts_with('{')).unwrap()).unwrap();
        assert_eq!(
            r["recorded_decisions"][0]["disposition"],
            if key.is_some() {
                "live_key_invalid"
            } else {
                "live_key_missing"
            }
        );
        assert_eq!(r["inference_profile"], "deepseek-genai-responses-low-v1");
        assert_eq!(r["effect_count"], 0);
        assert_eq!(
            server.count(),
            0,
            "missing/invalid infrastructure key cannot fall back to fixture"
        );
        let d = uuid::Uuid::parse_str(r["decision_one"].as_str().unwrap()).unwrap();
        assert_eq!(
            owner_counts(d)[1],
            0,
            "no SDK call marker before key validation"
        );
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn sdk_profiles_are_refused_by_legacy_wire_and_protocol_fixture_constructors() {
    for name in [
        "fixture-genai-responses-v1",
        "deepseek-genai-responses-low-v1",
    ] {
        let p = serde_json::json!({"version":"m1_context_pack_autonomous_v2","run_profile":"autonomous-dns-fixture-v2",
            "execution_mode":"controlled_fixture","inference_profile":name,"references":[{"id":"asset_0"}],"manifests":[{"reference":"dns_udp"}]});
        assert!(
            duskweave::m1_reasoning::request::bind(&serde_json::to_vec(&p).unwrap(), None).is_err(),
            "legacy binder must reject SDK metadata"
        );
        let profile = duskweave::m1_inference_profile::lookup(name).unwrap();
        assert!(
            duskweave::m1_reasoning::fixture::ProtocolFixture::new(profile).is_err(),
            "legacy protocol fixture must reject SDK metadata"
        );
    }
}

#[test]
fn sdk_permission_cannot_underreserve_the_fixed_request_entity_ceiling() {
    let _guard = native::db();
    refresh();
    let mut raw = consent::registration(
        duskweave::mission::EngagementId(uuid::Uuid::from_u128(0x21)),
        duskweave::mission::CampaignId(uuid::Uuid::from_u128(0x22)),
    );
    fixture::configure_permission(&mut raw);
    for cap in [1, 16383] {
        raw["m1_inference_permission"]["operation_limits"]["request_bytes"] =
            serde_json::json!(cap);
        assert!(
            duskweave::input::parse_register(&serde_json::to_vec(&raw).unwrap()).is_err(),
            "SDK request ceiling must fit admitted permission"
        );
        let matched: bool = native::client("DW_INFERENCE_BROKER_DATABASE_URL")
            .query_one(
                "SELECT execution.inference_profile_matches($1)",
                &[&raw["m1_inference_permission"]],
            )
            .unwrap()
            .get(0);
        assert!(
            !matched,
            "restricted SQL must independently refuse underfunded SDK requests"
        );
    }
}

#[path = "support/m1_lifecycle_wire.rs"]
#[allow(dead_code)]
mod wire;

fn owner_counts(d: uuid::Uuid) -> [i64; 3] {
    let row=native::admin().query_one("SELECT (SELECT count(*) FROM execution.inference_reservations WHERE operation_id=$1), (SELECT count(*) FROM execution.inference_calls WHERE d=$1), (SELECT count(*) FROM execution.inference_calls WHERE d=$1 AND state='may_send_uncertain')",&[&d]).unwrap();
    [row.get(0), row.get(1), row.get(2)]
}

#[path = "m1_genai_integration/context.rs"]
mod context;
#[path = "m1_genai_integration/episode.rs"]
mod episode;
#[path = "m1_genai_integration/failures.rs"]
mod failures;
#[path = "m1_genai_integration/termination.rs"]
mod termination;

// Keep externally selected test identities and libtest ordering stable.
#[test]
fn sdk_real_cli_completes_feedback_and_fresh_key_free_recovery() {
    episode::sdk_real_cli_completes_feedback_and_fresh_key_free_recovery();
}

#[test]
fn sdk_eight_seed_producer_preserves_real_entity_and_intent_capacity() {
    episode::sdk_eight_seed_producer_preserves_real_entity_and_intent_capacity();
}

#[test]
fn sdk_d2_omits_optional_summary_under_admitted_cap_before_removing_candidates() {
    episode::sdk_d2_omits_optional_summary_under_admitted_cap_before_removing_candidates();
}

#[test]
fn sdk_context_reaches_real_owner_pack_before_reservation() {
    context::sdk_context_reaches_real_owner_pack_before_reservation();
}

#[test]
fn sdk_cli_defer_repair_exhaustion_incomplete_protocol_and_unknown_never_replay() {
    failures::sdk_cli_defer_repair_exhaustion_incomplete_protocol_and_unknown_never_replay();
}

#[test]
fn sdk_stale_record_preserves_observe_intent_and_blocks_restricted_effect_release() {
    failures::sdk_stale_record_preserves_observe_intent_and_blocks_restricted_effect_release();
}

#[test]
fn sdk_stop_before_release_keeps_zero_calls_and_effects() {
    termination::sdk_stop_before_release_keeps_zero_calls_and_effects();
}

#[test]
fn sdk_observed_send_stop_lease_loss_and_parent_loss_terminate_without_replay() {
    termination::sdk_observed_send_stop_lease_loss_and_parent_loss_terminate_without_replay();
}

#[test]
fn sdk_post_send_assertion_cleanup_settles_owned_processes_response_and_files() {
    termination::sdk_post_send_assertion_cleanup_settles_owned_processes_response_and_files();
}
