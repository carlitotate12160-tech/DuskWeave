//! Successor contracts use fresh explicit profiles; legacy records retain their wire semantics.
use duskweave::input::parse_register;
use duskweave::mission::{CampaignId, EngagementId};
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_terrain_context.rs"]
mod terrain;
use terrain::terrain_support::native;
#[path = "support/m1_inference_lifecycle.rs"]
mod lifecycle;
#[path = "support/m1_run.rs"]
mod run_support;

#[test]
fn t7_reported_guard_is_optional_closed_evidence_separate_from_parent_cause() {
    let raw = json!({"version":"m1_run_phase_diagnostic_v2","run_id":Uuid::from_u128(17),
        "phase":"decision_one","cause":"child_exit","cause_source":"parent_observed",
        "child_error":null,"exit_code":124,"readiness_observed":true,"elapsed_ms":61950,
        "reported_guard":"transport_hard_deadline"});
    let diagnostic: duskweave::m1_run::PhaseDiagnostic = serde_json::from_value(raw.clone())
        .expect("a received closed report must survive the diagnostic boundary");
    diagnostic.validate().unwrap();
    for value in [
        json!("timeout"),
        json!("synthetic-sensitive-sentinel"),
        json!(3),
        Value::Null,
    ] {
        let mut bad = raw.clone();
        bad["reported_guard"] = value;
        assert!(serde_json::from_value::<duskweave::m1_run::PhaseDiagnostic>(bad).is_err());
    }
    let mut absent = raw;
    absent.as_object_mut().unwrap().remove("reported_guard");
    serde_json::from_value::<duskweave::m1_run::PhaseDiagnostic>(absent)
        .unwrap()
        .validate()
        .unwrap();
}

fn successor(profile: &str, recipient: &str) -> Value {
    let mut raw = consent::registration(
        EngagementId(Uuid::from_u128(1)),
        CampaignId(Uuid::from_u128(2)),
    );
    let p = &mut raw["m1_inference_permission"];
    p["version"] = json!("m1_inference_permission_v4");
    p["profile"] = json!(profile);
    p["recipient"] = json!(recipient);
    p["execution_mode"] = json!("controlled_fixture");
    p["run_profile"] = json!("autonomous-dns-fixture-v2");
    p["field_classes"] = json!([
        "mission_question_constraints",
        "scoped_opaque_references",
        "owner_qualified_summaries",
        "admitted_operator_premises",
        "manifest_availability",
        "omissions_unknown",
        "capacity",
        "admitted_repair_fields_codes",
        "admitted_campaign_decision_feedback"
    ]);
    p["campaign_limits"] = json!({"calls":4,"request_bytes":65536,"response_bytes":262144,"output_token_capacity":16384,"wall_time_ms":258000});
    for (name, value) in [
        ("context_bytes", 8192),
        ("json_depth", 32),
        ("decision_wall_time_ms", 129000),
        ("per_call_wall_time_ms", 62000),
        ("soft_completion_ms", 60000),
        ("shared_wall_time_ms", 124000),
        ("phase_wall_time_ms", 134000),
        ("cancel_grace_ms", 2000),
        ("accounting_window_ms", 3000),
    ] {
        p["operation_limits"][name] = json!(value);
    }
    raw
}

#[test]
fn successor_permission_admits_only_the_three_explicit_bound_profiles() {
    for (profile, recipient) in [
        ("fixture-responses-v1", "urn:duskweave:fixture:responses:v1"),
        (
            "fixture-chat-completions-v1",
            "urn:duskweave:fixture:chat-completions:v1",
        ),
        (
            "deepseek-responses-reasoning-v1",
            "https://api.deepseek.com/responses",
        ),
    ] {
        let raw = successor(profile, recipient);
        assert!(
            parse_register(&serde_json::to_vec(&raw).unwrap()).is_ok(),
            "a fresh v4 permission with the selected finite allocation must register: {profile}"
        );
        for (field, value) in [
            ("profile", json!("unknown")),
            ("recipient", json!("https://unapproved.invalid")),
            ("run_profile", json!("autonomous-dns-fixture-v1")),
            ("version", json!("m1_inference_permission_v3")),
        ] {
            let mut bad = raw.clone();
            bad["m1_inference_permission"][field] = value;
            assert!(
                parse_register(&serde_json::to_vec(&bad).unwrap()).is_err(),
                "{field}"
            );
        }
    }
}

#[test]
fn successor_permission_refuses_missing_or_rebound_timer_values() {
    let raw = successor("fixture-responses-v1", "urn:duskweave:fixture:responses:v1");
    assert!(parse_register(&serde_json::to_vec(&raw).unwrap()).is_ok());
    for name in [
        "soft_completion_ms",
        "per_call_wall_time_ms",
        "shared_wall_time_ms",
        "decision_wall_time_ms",
        "phase_wall_time_ms",
        "cancel_grace_ms",
        "accounting_window_ms",
    ] {
        for value in [json!(0), json!(-1), json!(1e300), Value::Null, json!(60001)] {
            let mut bad = raw.clone();
            bad["m1_inference_permission"]["operation_limits"][name] = value;
            assert!(
                parse_register(&serde_json::to_vec(&bad).unwrap()).is_err(),
                "{name}"
            );
        }
        let mut bad = raw.clone();
        bad["m1_inference_permission"]["operation_limits"]
            .as_object_mut()
            .unwrap()
            .remove(name);
        assert!(
            parse_register(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "missing {name}"
        );
    }
}

#[test]
fn successor_run_requires_explicit_profile_and_preserves_it_in_custody() {
    let id = Uuid::from_u128(1);
    let raw = json!({"version":"m1_autonomous_run_request_v2","run_id":id,
        "run_profile":"autonomous-dns-fixture-v2","execution_mode":"controlled_fixture",
        "inference_profile":"fixture-chat-completions-v1","deadline_at_ms":1700000400000i64,
        "context":{"version":"m1_context_prepare_request_v1","mission":{
            "version":"m1_inference_reservation_request_v1","engagement_id":id,"campaign_id":id,
            "operator_ref":id,"expected_mission_revision":1,"permission_id":id,"goal_ref":id,
            "exercise_mode":"blind"},"candidates":[{"asset_ref":id,"priority":"high"}]},
        "seed_names":[{"asset_ref":id,"name":"www.example.invalid"}]});
    let parsed = duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&raw).unwrap());
    assert!(
        parsed.is_ok(),
        "v2 run must admit the explicit selected profile"
    );
    assert_eq!(serde_json::to_value(parsed.unwrap()).unwrap(), raw);
    let mut missing = raw.clone();
    missing.as_object_mut().unwrap().remove("inference_profile");
    assert!(duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&missing).unwrap()).is_err());
    let mut old = raw;
    old["version"] = json!("m1_autonomous_run_request_v1");
    assert!(duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&old).unwrap()).is_err());
}

#[test]
fn both_successor_protocols_bind_the_same_pack_and_conditional_repair() {
    use duskweave::m1_reasoning::request;
    for profile in ["fixture-responses-v1", "fixture-chat-completions-v1"] {
        let p = json!({"version":"m1_context_pack_autonomous_v2","run_profile":"autonomous-dns-fixture-v2",
            "execution_mode":"controlled_fixture","inference_profile":profile,
            "references":[{"id":"asset_0"}],"manifests":[{"reference":"dns_udp"}]});
        let pack = serde_json::to_string(&p).unwrap();
        for repair in [None, Some(&["proposal_shape"][..])] {
            let bound = request::bind(pack.as_bytes(), repair).unwrap();
            assert!(
                request::verify_binding(
                    &pack,
                    std::str::from_utf8(&bound.configuration).unwrap(),
                    std::str::from_utf8(&bound.body).unwrap(),
                    &bound.digest,
                    repair.is_some()
                )
                .is_ok(),
                "both real codecs must verify the bound request: {profile}"
            );
            let mut config: Value = serde_json::from_slice(&bound.configuration).unwrap();
            config["recipient"] = json!("urn:unapproved");
            let forged = serde_json::to_string(&config).unwrap();
            let forged_digest =
                request::compute_binding(pack.as_bytes(), forged.as_bytes(), &bound.body);
            assert!(
                request::verify_binding(
                    &pack,
                    &forged,
                    std::str::from_utf8(&bound.body).unwrap(),
                    &forged_digest,
                    repair.is_some()
                )
                .is_err(),
                "a recomputed hash grants no profile rebinding"
            );
        }
    }
}

#[test]
fn t1_responses_real_cli_completes_feedback_and_key_free_recovery() {
    assert_real_cli_feedback(0x1c000, "fixture-responses-v1");
}

#[test]
fn t1_chat_completions_real_cli_completes_feedback_and_key_free_recovery() {
    assert_real_cli_feedback(0x1c001, "fixture-chat-completions-v1");
}

fn assert_real_cli_feedback(slot: u128, profile: &str) {
    let _guard = native::db();
    run_support::refresh();
    let q = lifecycle::registered(slot, profile);
    let path = run_support::path(&q);
    let root = run_support::control_root();
    let mut command = run_support::command();
    command.env("DW_M1_RUN_CONTROL_ROOT", &root).args([
        "m1-run",
        "--input",
        path.to_str().unwrap(),
        "--timing-profile",
        "reasoning-bounded-v1",
    ]);
    let output = lifecycle::readiness_cli(&q, command);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "real successor CLI must complete {profile}: {text}"
    );
    let receipt: Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["decision_count"], 2);
    assert_eq!(receipt["decision_history_count"], 2);
    assert_eq!(receipt["effect_count"], 1);
    assert_eq!(receipt["terrain_history_count"], 1);
    assert_eq!(receipt["inference_profile"], profile);
    assert_eq!(receipt["recorded_decisions"].as_array().unwrap().len(), 2);
    lifecycle::assert_bound_calls(&q, profile);
    for operation in ["m1-run-inspect", "m1-run-recover", "m1-run-recover"] {
        let (ok, r) = run_support::cli(operation, "--run", q["run_id"].as_str().unwrap());
        assert!(ok, "{r}");
        assert_eq!(r["effect_count"], 1);
        assert_eq!(r["decision_count"], 2);
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn t3_successor_stale_record_retains_the_actual_observe_intent() {
    let id = Uuid::from_u128(1);
    let proposal = json!({"version":"m1_inference_proposal_v1","hypothesis":"routing hypothesis",
        "considered_alternative":"routing unavailable","references":["terrain_0"],
        "expected_observation":"bounded observation","falsifying_observation":"missing binding",
        "uncertainty":"TTL expired","rationale":"retain historical reasoning",
        "next_intent":{"category":"observe_more","references":["asset_0"],"manifest_reference":"dns_udp"}});
    let raw = json!({"kind":"reasoning_decision","occurred_at_ms":1000,"recorded_at_ms":3000,
        "source_mode":"blind","provenance":[],"event":id,"engagement":id,"campaign":id,"decision":id,
        "producer":"cognition","version":2,"mission_revision":1,"permission":id,
        "call_ids":[],"bindings":[],"proposal":proposal,"disposition":"recorded_stale_premises",
        "publication_obligation":"trajectory.reasoning_history.v1"});
    let result = duskweave::m1_reasoning::postgres::decode(raw.clone());
    assert!(
        result.is_ok(),
        "safe successor history must retain stale reasoning"
    );
    assert_eq!(serde_json::to_value(result.unwrap()).unwrap(), raw);
    let mut forged_legacy = raw;
    forged_legacy["version"] = json!(1);
    assert!(
        duskweave::m1_reasoning::postgres::decode(forged_legacy).is_err(),
        "legacy cannot acquire successor stale disposition"
    );
}

#[test]
fn t1_inspection_orders_decisions_causally_when_the_first_uuid_sorts_last() {
    use duskweave::m1_run::RunPhase;
    let _guard = native::db();
    run_support::refresh();
    let q = lifecycle::registered(0xe620, "fixture-responses-v1");
    let original = run_support::start_only(&q);
    let first = original.decision_one.max(original.decision_two);
    let second = original.decision_one.min(original.decision_two);
    assert!(
        first > second,
        "the per-run pair must preserve reverse UUID ordering"
    );
    println!(
        "causal_uuid_pair run={} first={first} second={second}",
        original.request.run_id
    );
    // Private setup selects the adversarial UUID arrangement before any phase or owner input.
    native::admin()
        .execute(
            "UPDATE execution.autonomous_runs SET decision_one=$1,decision_two=$2 WHERE run_id=$3",
            &[&first, &second, &original.request.run_id],
        )
        .unwrap();
    let rec = run_support::broker_store()
        .inspect(original.request.run_id)
        .unwrap();
    let lease = run_support::LeaseCustodian::start(&rec, run_support::broker_store());
    lifecycle::seed_claim(&rec, 60);
    let prepared = lifecycle::prepare(&rec, RunPhase::DecisionTwo);
    let profile = duskweave::m1_inference_profile::lookup("fixture-responses-v1").unwrap();
    let receipt = lifecycle::infer(
        &rec,
        &prepared,
        &mut duskweave::m1_reasoning::fixture::ProtocolFixture::new(profile).unwrap(),
    );
    assert_eq!(receipt.history, "completed");
    let mut store = run_support::broker_store();
    // Active-work renewal ends before the terminal phase revokes its lease.
    lease.finish().unwrap();
    store
        .complete_phase(rec.request.run_id, RunPhase::DecisionTwo)
        .unwrap();
    for operation in ["m1-run-inspect", "m1-run-recover", "m1-run-recover"] {
        let (ok, summary) = run_support::cli(operation, "--run", q["run_id"].as_str().unwrap());
        assert!(ok, "{summary}");
        assert_eq!(
            summary["recorded_decisions"][0]["decision"],
            json!(first),
            "inspection must order the actual first decision before feedback, independent of UUID sort"
        );
        assert_eq!(summary["recorded_decisions"][1]["decision"], json!(second));
        assert_eq!(summary["effect_count"], 1);
        assert_eq!(summary["decision_history_count"], 2);
    }
}

#[test]
fn t3_real_owner_ttl_expiry_and_integrity_controls_record_without_another_dns() {
    use duskweave::m1_reasoning::proposal::ProposalBasis;
    use duskweave::m1_run::RunPhase;
    let _guard = native::db();
    run_support::refresh();
    for (offset, mode) in [
        "ttl",
        "pre_release",
        "hold",
        "identity",
        "conflict",
        "fresh",
    ]
    .into_iter()
    .enumerate()
    {
        let q = lifecycle::registered(0xe100 + offset as u128, "fixture-responses-v1");
        let rec = run_support::start_only(&q);
        let lease = run_support::LeaseCustodian::start(&rec, run_support::broker_store());
        let original = lifecycle::seed_claim(
            &rec,
            if mode == "ttl" || mode == "pre_release" {
                4
            } else {
                60
            },
        );
        let prepared = lifecycle::prepare(&rec, RunPhase::DecisionTwo);
        assert_eq!(
            prepared.pack.terrain_dependencies().len(),
            1,
            "owner view: {}",
            serde_json::from_str::<Value>(prepared.pack.utf8()).unwrap()["owner_views"]
        );
        let dep = prepared.pack.terrain_dependencies()[0].clone();
        assert_eq!(dep.perceived_at, original["perceived_at"].as_i64().unwrap());
        let e = prepared.scope.engagement;
        let c = prepared.scope.campaign;
        let mut provider = lifecycle::DelayedFixture {
            inner: duskweave::m1_reasoning::fixture::ProtocolFixture::new(
                duskweave::m1_inference_profile::lookup("fixture-responses-v1").unwrap(),
            )
            .unwrap(),
            entries: 0,
            before_release: mode == "pre_release",
            after_release: Some(Box::new(move || {
                if mode == "ttl" {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
                let mut admin = native::admin();
                match mode {
                    "hold" => {
                        admin.execute("INSERT INTO terrain.integrity_conflicts SELECT engagement_id,campaign_id,operation_id,claim_id,perceived_at,true,'input_mismatch' FROM terrain.current_claims WHERE engagement_id=$1 AND campaign_id=$2",&[&e,&c]).unwrap();
                    }
                    "identity" => {
                        admin.execute("UPDATE terrain.current_claims SET evaluation_id=gen_random_uuid() WHERE engagement_id=$1 AND campaign_id=$2",&[&e,&c]).unwrap();
                    }
                    "conflict" => {
                        admin.execute("UPDATE terrain.current_claims SET conflicted=true,claim_status='conflicted' WHERE engagement_id=$1 AND campaign_id=$2",&[&e,&c]).unwrap();
                    }
                    _ => {}
                }
            })),
        };
        let receipt = lifecycle::infer(&rec, &prepared, &mut provider);
        assert_eq!(
            provider.entries,
            if mode == "pre_release" { 0 } else { 1 },
            "{mode}"
        );
        assert!(!receipt.dispatch_granted);
        if mode == "ttl" {
            assert_eq!(receipt.proposal_disposition, "recorded_stale_premises");
            assert_eq!(receipt.history, "completed");
            assert_eq!(
                receipt.proposal.as_ref().unwrap().next_intent.category,
                duskweave::m1_reasoning::proposal::Category::Retain
            );
            for op in ["m1-run-inspect", "m1-run-recover", "m1-run-recover"] {
                let (ok, summary) = run_support::cli(op, "--run", q["run_id"].as_str().unwrap());
                assert!(ok, "{summary}");
                assert_eq!(summary["effect_count"], 1);
                assert_eq!(summary["decision_history_count"], 2);
            }
            let state:Value=native::client("DW_INFERENCE_BROKER_DATABASE_URL").query_one("SELECT jsonb_build_object('states',jsonb_agg(state),'usage',jsonb_agg(usage)) FROM execution.inference_calls WHERE d=$1",&[&rec.decision_two]).unwrap().get(0);
            assert_eq!(state["states"], json!(["completed"]));
            assert!(state["usage"][0]["wall_time_ms"].as_u64().unwrap() >= 5000);
        } else if mode == "fresh" {
            assert_eq!(receipt.proposal_disposition, "validated_proposal");
            assert_eq!(receipt.history, "completed");
        } else {
            assert_ne!(receipt.proposal_disposition, "validated_proposal", "{mode}");
            assert_ne!(
                receipt.proposal_disposition, "recorded_stale_premises",
                "{mode}"
            );
        }
        let mut owner = lifecycle::basis(
            &prepared,
            std::time::Instant::now() + std::time::Duration::from_secs(10),
        );
        if mode != "fresh" {
            assert!(
                owner.before_provider_release().is_err(),
                "expiry or changed owner must refuse a new release"
            );
        }
        let count: i64 = native::admin()
            .query_one(
                "SELECT count(*) FROM execution.fixture_dns_attempts WHERE run_id=$1",
                &[&rec.request.run_id],
            )
            .unwrap()
            .get(0);
        assert_eq!(count, 1, "stale result or recovery released another DNS");
        lease.finish().unwrap();
    }
}

#[test]
fn t9_real_producers_fit_both_protocols_and_restricted_sql_rejects_recomputed_forgeries() {
    use duskweave::m1_run::RunPhase;
    let _guard = native::db();
    run_support::refresh();
    let mut cases = Vec::new();
    for (i, profile) in ["fixture-responses-v1", "fixture-chat-completions-v1"]
        .into_iter()
        .enumerate()
    {
        let mut exact_width = 0;
        for (j, kind) in [
            "minimum",
            "eight_seeds",
            "exact_limit",
            "eight_seeds_quoted",
            "quote_backslash",
            "multibyte",
        ]
        .into_iter()
        .enumerate()
        {
            let q = lifecycle::registered_with(0xe200 + (i * 10 + j) as u128, profile, |raw| {
                if kind.starts_with("eight_seeds") || kind == "exact_limit" {
                    lifecycle::eight_seeds(raw);
                }
            });
            let rec = run_support::start_only(&q);
            let lease = run_support::LeaseCustodian::start(&rec, run_support::broker_store());
            let hypothesis = match kind {
                "quote_backslash" | "eight_seeds_quoted" => "\"\\".repeat(256),
                "multibyte" => "é".repeat(512),
                "exact_limit" => "x".repeat(exact_width),
                _ => "A bounded fixture hypothesis.".into(),
            };
            let mut provider = lifecycle::WireFixture {
                hypothesis,
                fault: "none",
                entries: 0,
            };
            lifecycle::seed_claim_with(&rec, 60, &mut provider);
            assert_eq!(provider.entries, 1);
            let first: String = native::client("DW_INFERENCE_BROKER_DATABASE_URL")
                .query_one(
                    "SELECT p FROM execution.inference_calls WHERE d=$1",
                    &[&rec.decision_one],
                )
                .unwrap()
                .get(0);
            let prepared = lifecycle::prepare(&rec, RunPhase::DecisionTwo);
            let second: Value = serde_json::from_str(prepared.pack.utf8()).unwrap();
            assert_eq!(second["predecessor"]["result_ref"], json!(rec.result));
            assert_eq!(prepared.pack.terrain_dependencies().len(), 1);
            if kind == "eight_seeds" {
                assert_eq!(second["candidates"].as_array().unwrap().len(), 8);
            }
            if kind == "eight_seeds" {
                exact_width =
                    8192 - prepared.pack.utf8().len() + "A bounded fixture hypothesis.".len();
                assert!(
                    (1..=1024).contains(&exact_width),
                    "exact producer width must remain within the text limit"
                );
            }
            if kind == "exact_limit" {
                assert_eq!(
                    prepared.pack.utf8().len(),
                    8192,
                    "real producer exact UTF-8 limit"
                );
                let mut over = prepared.pack.utf8().as_bytes().to_vec();
                over.push(b' ');
                assert!(duskweave::m1_reasoning::request::reference_ids(&over).is_err());
            }
            if kind == "eight_seeds_quoted" {
                assert_eq!(
                    second["candidates"].as_array().unwrap().len(),
                    8,
                    "optional summary cannot displace required seeds"
                );
                assert_eq!(
                    second["predecessor"]["hypothesis_summary_omission"],
                    "byte_bound"
                );
            }
            cases.push(lifecycle::wire_case(
                &format!("{profile}_{kind}_first"),
                &first,
            ));
            cases.push(lifecycle::wire_case(
                &format!("{profile}_{kind}_feedback"),
                prepared.pack.utf8(),
            ));
            lifecycle::restricted_wire_checks(&rec, &prepared);
            lease.finish().unwrap();
        }
    }
    if std::env::var_os("DW_WRITE_LIFECYCLE_WIRE_EXAMPLES").is_some() {
        let artifact = json!({"version":"m1-inference-lifecycle-wire-v2","source":"real ContextPack/request producers and restricted SQL; synthetic fixture identities only","cases":cases});
        std::fs::write(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("docs/contracts/fixtures/m1-inference-lifecycle-wire-v2.json"),
            serde_json::to_vec_pretty(&artifact).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn t3_first_stale_owner_disposition_defers_normal_consumer_and_direct_sql_keeps_zero_dns() {
    use duskweave::m1_reasoning::{inference, postgres::PgDecisionStore};
    use duskweave::m1_run::{NextStep, RunPhase};
    let _guard = native::db();
    run_support::refresh();
    let q = lifecycle::registered(0xe310, "fixture-chat-completions-v1");
    let rec = run_support::start_only(&q);
    let lease = run_support::LeaseCustodian::start(&rec, run_support::broker_store());
    let prepared = lifecycle::prepare(&rec, RunPhase::DecisionOne);
    let mut broker = lifecycle::broker(&rec, &prepared);
    let mut basis = lifecycle::ExpiredAtRecording(lifecycle::basis(&prepared, broker.deadline));
    let mut provider = lifecycle::WireFixture {
        hypothesis: "A safe historical hypothesis.".into(),
        fault: "none",
        entries: 0,
    };
    let receipt = inference::infer(
        &mut broker,
        &mut provider,
        &mut basis,
        &mut PgDecisionStore {
            client: native::client("DW_COGNITION_DATABASE_URL"),
            input_hash: prepared.hash.clone(),
        },
        &mut duskweave::postgres_trajectory::PgTrajectory::new(native::client(
            "DW_TRAJECTORY_DATABASE_URL",
        )),
        inference::InferenceInput {
            scope: &prepared.scope,
            pack: prepared.pack.utf8().as_bytes(),
            event_id: run_support::consent_id(),
            occurred_at_ms: 1000,
            source_mode: "blind".into(),
        },
    );
    assert_eq!(receipt.proposal_disposition, "recorded_stale_premises");
    assert_eq!(receipt.history, "completed");
    assert_eq!(
        receipt.proposal.unwrap().next_intent.category,
        duskweave::m1_reasoning::proposal::Category::ObserveMore
    );
    let mut store = run_support::broker_store();
    assert_eq!(
        store.first_step(&rec).unwrap(),
        NextStep::Terminal("deferred")
    );
    assert!(store.first_proposal(&rec).is_err());
    store
        .complete_phase(rec.request.run_id, RunPhase::DecisionOne)
        .unwrap();
    store
        .start_phase(rec.request.run_id, RunPhase::Acquisition)
        .unwrap();
    store
        .claim_phase(
            rec.request.run_id,
            RunPhase::Acquisition,
            &run_support::release(),
        )
        .unwrap();
    assert!(
        store.reserve_effect(rec.request.run_id, 0).is_err(),
        "restricted SQL independently rejects stale disposition"
    );
    lease.finish().unwrap();
    store.terminal(rec.request.run_id, "deferred").unwrap();
    for op in ["m1-run-inspect", "m1-run-recover", "m1-run-recover"] {
        let (ok, summary) = run_support::cli(op, "--run", q["run_id"].as_str().unwrap());
        assert!(ok, "{summary}");
        assert_eq!(summary["effect_count"], 0);
        assert_eq!(summary["status"], "deferred");
        assert_eq!(summary["decision_history_count"], 1);
        assert_eq!(
            summary["recorded_decisions"][0]["disposition"],
            "recorded_stale_premises"
        );
        assert_eq!(
            summary["recorded_decisions"][0]["proposal"]["next_intent"]["category"],
            "observe_more"
        );
        assert_eq!(
            summary["recorded_decisions"][0]["current_use"],
            "blocked_stale_premises"
        );
    }
    assert_eq!(provider.entries, 1);
}

#[test]
fn t7_restricted_successor_diagnostic_keeps_reports_separate_and_rejects_forged_versions() {
    use duskweave::m1_run::RunPhase;
    let _guard = native::db();
    run_support::refresh();
    let q = lifecycle::registered(0xe320, "fixture-responses-v1");
    let rec = run_support::start_only(&q);
    let mut store = run_support::broker_store();
    store
        .start_phase(rec.request.run_id, RunPhase::DecisionOne)
        .unwrap();
    let raw = json!({"version":"m1_run_phase_diagnostic_v2","run_id":rec.request.run_id,"phase":"decision_one",
        "cause":"child_exit","cause_source":"parent_observed","child_error":null,"exit_code":124,
        "readiness_observed":true,"elapsed_ms":61714,"reported_guard":"permit_expiry"});
    let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    for (field, value) in [
        ("reported_guard", json!("timeout")),
        ("reported_guard", Value::Null),
        ("cause_source", json!("child_report")),
        ("version", json!("m1_run_phase_diagnostic_v1")),
    ] {
        let mut bad = raw.clone();
        bad[field] = value;
        let mut tx = broker.transaction().unwrap();
        assert!(
            tx.query_one("SELECT execution.run_phase_diagnostic($1)", &[&bad])
                .is_err()
        );
        tx.rollback().unwrap();
    }
    assert!(
        native::client("DW_COGNITION_DATABASE_URL")
            .query_one("SELECT execution.run_phase_diagnostic($1)", &[&raw])
            .is_err()
    );
    for _ in 0..2 {
        broker
            .query_one("SELECT execution.run_phase_diagnostic($1)", &[&raw])
            .unwrap();
    }
    let summary = run_support::broker_store()
        .summary(rec.request.run_id)
        .unwrap();
    assert_eq!(summary["diagnostics"][0], raw);
}

#[test]
fn t9_generated_successor_examples_reproduce_exact_utf8_and_bound_wires() {
    let artifact: Value = serde_json::from_str(include_str!(
        "../docs/contracts/fixtures/m1-inference-lifecycle-wire-v2.json"
    ))
    .unwrap();
    assert_eq!(artifact["cases"].as_array().unwrap().len(), 24);
    for case in artifact["cases"].as_array().unwrap() {
        assert_eq!(
            lifecycle::wire_case(
                case["name"].as_str().unwrap(),
                case["pack"].as_str().unwrap()
            ),
            *case
        );
    }
}

#[test]
fn t6_successor_failures_never_retry_and_only_real_repairable_codec_output_opens_repair() {
    use duskweave::m1_reasoning::{inference, postgres::PgDecisionStore};
    use duskweave::m1_run::RunPhase;
    let _guard = native::db();
    run_support::refresh();
    for (i, fault) in ["provider", "unsafe", "publication", "history", "repair"]
        .into_iter()
        .enumerate()
    {
        let q = lifecycle::registered(0xe400 + i as u128, "fixture-chat-completions-v1");
        let rec = run_support::start_only(&q);
        let lease = run_support::LeaseCustodian::start(&rec, run_support::broker_store());
        let prepared = lifecycle::prepare(&rec, RunPhase::DecisionOne);
        let mut broker = lifecycle::broker(&rec, &prepared);
        let mut basis = lifecycle::basis(&prepared, broker.deadline);
        let mut provider = lifecycle::WireFixture {
            hypothesis: if fault == "unsafe" {
                "password synthetic sentinel"
            } else {
                "A bounded routing hypothesis."
            }
            .into(),
            fault,
            entries: 0,
        };
        let mut store = lifecycle::FaultStore {
            inner: PgDecisionStore {
                client: native::client("DW_COGNITION_DATABASE_URL"),
                input_hash: prepared.hash.clone(),
            },
            fail: fault == "publication",
        };
        let mut history = lifecycle::FaultHistory {
            inner: duskweave::postgres_trajectory::PgTrajectory::new(native::client(
                "DW_TRAJECTORY_DATABASE_URL",
            )),
            fail: fault == "history",
        };
        let receipt = inference::infer(
            &mut broker,
            &mut provider,
            &mut basis,
            &mut store,
            &mut history,
            inference::InferenceInput {
                scope: &prepared.scope,
                pack: prepared.pack.utf8().as_bytes(),
                event_id: run_support::consent_id(),
                occurred_at_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
                source_mode: "blind".into(),
            },
        );
        let expected = if fault == "repair" { 2 } else { 1 };
        assert_eq!(provider.entries, expected, "{fault}");
        let rows = native::client("DW_INFERENCE_BROKER_DATABASE_URL")
            .query(
                "SELECT call,binding,state,usage FROM execution.inference_calls WHERE d=$1",
                &[&rec.decision_one],
            )
            .unwrap();
        assert_eq!(rows.len(), expected);
        if fault == "repair" {
            assert_ne!(rows[0].get::<_, Uuid>(0), rows[1].get::<_, Uuid>(0));
            assert_ne!(rows[0].get::<_, String>(1), rows[1].get::<_, String>(1));
            assert_eq!(receipt.proposal_disposition, "validated_proposal");
        }
        if fault == "provider" {
            assert_eq!(receipt.consumption, "uncertain_charged");
        } else {
            for row in rows {
                let usage: Value = row.get(3);
                assert_eq!(usage["input_tokens"], Value::Null);
                assert_eq!(usage["output_tokens"], Value::Null);
            }
        }
        for _ in 0..2 {
            let (ok, summary) =
                run_support::cli("m1-run-recover", "--run", q["run_id"].as_str().unwrap());
            assert!(ok, "{summary}");
            assert_eq!(summary["effect_count"], 0);
        }
        let count: i64 = native::client("DW_INFERENCE_BROKER_DATABASE_URL")
            .query_one(
                "SELECT count(*) FROM execution.inference_calls WHERE d=$1",
                &[&rec.decision_one],
            )
            .unwrap()
            .get(0);
        assert_eq!(
            count, expected as i64,
            "database-only recovery created a replacement call"
        );
        assert_eq!(provider.entries, expected);
        lease.finish().unwrap();
    }
}

#[test]
fn t5_profile_matrix_and_raw_sql_run_admission_are_closed_and_share_one_window_origin() {
    use duskweave::m1_run::RunPhase;
    let _guard = native::db();
    run_support::refresh();
    for (i, profile) in [
        "fixture-responses-v1",
        "fixture-chat-completions-v1",
        "deepseek-responses-reasoning-v1",
    ]
    .into_iter()
    .enumerate()
    {
        let q = lifecycle::registered(0xe500 + i as u128, profile);
        let mut client = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
        for field in [
            "version",
            "run_profile",
            "inference_profile",
            "execution_mode",
            "context",
            "deadline_at_ms",
        ] {
            let mut bad = q.clone();
            bad.as_object_mut().unwrap().remove(field);
            let mut tx = client.transaction().unwrap();
            assert!(
                tx.query_one(
                    "SELECT execution.run_start($1,$2)",
                    &[&bad, &run_support::release()]
                )
                .is_err(),
                "raw SQL accepted absent {field}"
            );
            tx.rollback().unwrap();
        }
        let rec = run_support::start_only(&q);
        let lease = run_support::LeaseCustodian::start(&rec, run_support::broker_store());
        let prepared = lifecycle::prepare(&rec, RunPhase::DecisionOne);
        let b =
            duskweave::m1_reasoning::request::bind(prepared.pack.utf8().as_bytes(), None).unwrap();
        let config: Value = serde_json::from_slice(&b.configuration).unwrap();
        for (key, value) in [
            ("connect_time_ms", 5000),
            ("soft_completion_ms", 60000),
            ("per_call_wall_time_ms", 62000),
            ("shared_wall_time_ms", 124000),
            ("decision_wall_time_ms", 129000),
            ("phase_wall_time_ms", 134000),
            ("cancel_grace_ms", 2000),
            ("accounting_window_ms", 3000),
        ] {
            assert_eq!(config["limits"][key], value);
        }
        assert_eq!(
            serde_json::from_str::<Value>(prepared.pack.utf8()).unwrap()["remaining_capacity"]["required_for_full_decision"]
                ["wall_time_ms"],
            129000
        );
        let row=client.query_one("SELECT contract#>'{fields,m1_inference_permission}' FROM mission.registration_outbox WHERE engagement_id=$1 AND campaign_id=$2",
            &[&prepared.scope.engagement,&prepared.scope.campaign]).unwrap();
        let permission: Value = row.get(0);
        let good: bool = client
            .query_one(
                "SELECT execution.inference_profile_matches($1)",
                &[&permission],
            )
            .unwrap()
            .get(0);
        assert!(good);
        for key in [
            "soft_completion_ms",
            "per_call_wall_time_ms",
            "shared_wall_time_ms",
            "decision_wall_time_ms",
            "phase_wall_time_ms",
        ] {
            for value in [
                Value::Null,
                json!(0),
                json!(-1),
                json!(1e99),
                json!(60000.0),
            ] {
                let mut bad = permission.clone();
                bad["operation_limits"][key] = value;
                let accepted: bool = client
                    .query_one("SELECT execution.inference_profile_matches($1)", &[&bad])
                    .unwrap()
                    .get(0);
                assert!(!accepted, "SQL accepted an incoherent {key}");
            }
        }
        let end: i64 = client
            .query_one(
                "SELECT execution.inference_window_ms($1,$2,$3)",
                &[
                    &prepared.scope.engagement,
                    &prepared.scope.campaign,
                    &prepared.scope.decision,
                ],
            )
            .unwrap()
            .get(0);
        std::thread::sleep(std::time::Duration::from_millis(50));
        let again: i64 = client
            .query_one(
                "SELECT execution.inference_window_ms($1,$2,$3)",
                &[
                    &prepared.scope.engagement,
                    &prepared.scope.campaign,
                    &prepared.scope.decision,
                ],
            )
            .unwrap()
            .get(0);
        assert!(
            again <= end - 40,
            "a repeated owner read must consume the original shared clock, not return a fresh duration"
        );
        lease.finish().unwrap();
    }
}

#[test]
fn t8_populated_legacy_owner_records_survive_forward_migration_and_repeatable_refresh() {
    if !lifecycle::upgrade_worker() {
        return;
    }
    let _guard = native::db();
    lifecycle::assert_predecessor_catalog();
    run_support::refresh_legacy();
    lifecycle::assert_predecessor_catalog();
    let q = run_support::registered(0xe600);
    let rec = run_support::start_only(&q);
    let lease = run_support::LeaseCustodian::start(&rec, run_support::broker_store());
    run_support::complete_first(&rec);
    lease.finish().unwrap();
    let mut store = run_support::broker_store();
    store
        .terminal(rec.request.run_id, "capability_gap")
        .unwrap();
    let before = run_support::broker_store()
        .summary(rec.request.run_id)
        .unwrap();
    assert_eq!(before["decision_history_count"], 1);
    let payload: Value = native::client("DW_COGNITION_DATABASE_URL")
        .query_one(
            "SELECT payload FROM cognition.reasoning_decisions WHERE d=$1",
            &[&rec.decision_one],
        )
        .unwrap()
        .get(0);
    assert_eq!(payload["version"], 1);
    lifecycle::assert_predecessor_catalog();
    let snapshot = lifecycle::legacy_snapshot();
    for table in [
        "execution.autonomous_runs",
        "execution.inference_calls",
        "execution.native_input_bindings",
        "cognition.reasoning_decisions",
        "cognition.reasoning_outbox",
        "trajectory.reasoning_history",
    ] {
        assert_eq!(
            snapshot[table].as_array().unwrap().len(),
            1,
            "populated predecessor: {table}"
        );
    }
    assert!(
        !snapshot["execution.inference_reservations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for _ in 0..2 {
        native::admin()
            .batch_execute(include_str!("../migrations/0021_inference_lifecycle.sql"))
            .unwrap();
        let catalog = native::admin()
            .query_one(
                "SELECT to_regclass('execution.inference_decision_windows') IS NOT NULL,
            to_regprocedure('execution.inference_window_ms(uuid,uuid,uuid)') IS NOT NULL",
                &[],
            )
            .unwrap();
        assert!(catalog.get::<_, bool>(0) && catalog.get::<_, bool>(1));
        let after = run_support::broker_store()
            .summary(rec.request.run_id)
            .unwrap();
        assert_eq!(after["inference"], before["inference"]);
        assert_eq!(after["decision_history_count"], 1);
        let retained: Value = native::client("DW_COGNITION_DATABASE_URL")
            .query_one(
                "SELECT payload FROM cognition.reasoning_decisions WHERE d=$1",
                &[&rec.decision_one],
            )
            .unwrap()
            .get(0);
        assert_eq!(
            retained, payload,
            "forward migration changed a legacy decision"
        );
        for operation in ["m1-run-inspect", "m1-run-recover", "m1-run-recover"] {
            let (ok, summary) = run_support::cli(operation, "--run", q["run_id"].as_str().unwrap());
            assert!(ok, "{summary}");
            assert_eq!(summary["effect_count"], 0);
            assert_eq!(summary["decision_history_count"], 1);
            assert_eq!(summary["inference"], before["inference"]);
            assert_eq!(
                lifecycle::legacy_snapshot(),
                snapshot,
                "upgrade/recovery changed legacy owner rows or charges"
            );
        }
    }
    println!("t8_upgrade_worker_complete");
}

#[test]
fn t5_deepseek_cli_without_a_key_keeps_its_profile_and_never_falls_back_or_replays() {
    let _guard = native::db();
    run_support::refresh();
    let q = lifecycle::registered(0xe610, "deepseek-responses-reasoning-v1");
    let path = run_support::path(&q);
    let output = run_support::command()
        .env("DW_M1_RUN_CONTROL_ROOT", run_support::control_root())
        .args([
            "m1-run",
            "--input",
            path.to_str().unwrap(),
            "--timing-profile",
            "reasoning-bounded-v1",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    let receipt: Value = text
        .lines()
        .find(|line| line.starts_with('{'))
        .map(|line| serde_json::from_str(line).unwrap())
        .unwrap_or_else(|| panic!("missing run receipt: {text}"));
    assert_eq!(
        receipt["inference_profile"],
        "deepseek-responses-reasoning-v1"
    );
    assert_eq!(receipt["inference"], json!([]));
    assert_eq!(receipt["effect_count"], 0);
    assert_eq!(
        receipt["recorded_decisions"][0]["disposition"],
        "live_key_missing"
    );
    assert_eq!(
        receipt["recorded_decisions"][0]["current_use"],
        "not_admitted"
    );
    for _ in 0..2 {
        let (ok, after) =
            run_support::cli("m1-run-recover", "--run", q["run_id"].as_str().unwrap());
        assert!(ok, "{after}");
        assert_eq!(after["inference"], json!([]));
        assert_eq!(after["effect_count"], 0);
    }
    std::fs::remove_file(path).unwrap();
}
