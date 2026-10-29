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
#[test]
fn sdk_real_cli_completes_feedback_and_fresh_key_free_recovery() {
    assert_sdk_real_cli(0x2a100, |_| {});
}
#[path = "support/m1_lifecycle_wire.rs"]
#[allow(dead_code)]
mod wire;
#[test]
fn sdk_eight_seed_producer_preserves_real_entity_and_intent_capacity() {
    assert_sdk_real_cli(0x2a102, wire::eight_seeds);
}
fn assert_sdk_real_cli(
    slot: u128,
    configure: impl FnOnce(&mut serde_json::Value),
) -> Vec<serde_json::Value> {
    let _guard = native::db();
    refresh();
    let server = fixture::Server::start("127.0.0.1:18083", |r| {
        let wire: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        let pack = fixture::pack_in(&wire).expect("real SDK must carry immutable P");
        assert_eq!(wire["model"], "deepseek-flash");
        assert_eq!(wire["reasoning"]["effort"], "low");
        assert_eq!(wire["max_output_tokens"], 4096);
        assert_eq!(wire["store"], false);
        assert!(wire["stream"].as_bool().is_none_or(|v| !v));
        for field in [
            "temperature",
            "top_p",
            "previous_response_id",
            "conversation",
        ] {
            assert!(wire.get(field).is_none());
        }
        assert!(wire["tools"].as_array().is_none_or(|v| v.is_empty()));

        fixture::Reply::Length(
            200,
            serde_json::to_vec(&fixture::envelope(&fixture::proposal(&pack))).unwrap(),
        )
    });
    let q = registered_with(slot, configure);
    let path = run_support::path(&q);
    let root = run_support::control_root();
    let output = run_support::command()
        .env("DW_M1_RUN_CONTROL_ROOT", &root)
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
    assert!(
        output.status.success(),
        "safe CLI receipt: {text}; sends={} request_sizes={:?} wire={:?}",
        server.count(),
        server
            .requests
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.body.len())
            .collect::<Vec<_>>(),
        server.wire.lock().unwrap()
    );
    let receipt: serde_json::Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["decision_count"], 2);
    assert_eq!(receipt["decision_history_count"], 2);
    assert_eq!(receipt["effect_count"], 1);
    assert_eq!(receipt["terrain_history_count"], 1);
    assert_ne!(
        receipt["recorded_decisions"][0]["proposal"]["hypothesis"],
        receipt["recorded_decisions"][1]["proposal"]["hypothesis"]
    );
    assert_eq!(
        receipt["recorded_decisions"][1]["proposal"]["next_intent"]["category"],
        "retain"
    );
    assert_eq!(server.count(), 2, "D1 and D2 must each send exactly once");
    for operation in ["m1-run-inspect", "m1-run-recover", "m1-run-recover"] {
        let (ok, r) = run_support::cli(operation, "--run", q["run_id"].as_str().unwrap());
        assert!(ok, "{r}");
        assert_eq!(r["effect_count"], 1);
        assert_eq!(r["decision_count"], 2);
        assert_eq!(
            server.count(),
            2,
            "fresh recovery must never contact provider"
        );
    }
    eprintln!(
        "synthetic_provider_dns_fixture_episode run={} decisions={} decision_history={} effects={} terrain_history={} observed_provider_requests={} d1_intent={} d2_intent={}",
        q["run_id"],
        receipt["decision_count"],
        receipt["decision_history_count"],
        receipt["effect_count"],
        receipt["terrain_history_count"],
        server.count(),
        receipt["recorded_decisions"][0]["proposal"]["next_intent"]["category"],
        receipt["recorded_decisions"][1]["proposal"]["next_intent"]["category"]
    );
    let rows = native::admin()
        .query(
            "SELECT p,configuration,body,binding,usage FROM execution.inference_calls WHERE e=$1",
            &[
                &uuid::Uuid::parse_str(q["context"]["mission"]["engagement_id"].as_str().unwrap())
                    .unwrap(),
            ],
        )
        .unwrap();
    assert_eq!(rows.len(), 2);
    let mut observed_packs = Vec::new();
    for row in rows {
        let (p, i, b, h): (String, String, String, String) =
            (row.get(0), row.get(1), row.get(2), row.get(3));
        observed_packs.push(serde_json::from_str(&p).unwrap());
        assert!(b.is_empty());
        assert_eq!(
            duskweave::m1_reasoning::inference_intent::digest(p.as_bytes(), i.as_bytes()),
            h
        );
        assert!(!i.contains("synthetic-sdk-fixture-key"));
        let intent: serde_json::Value = serde_json::from_str(&i).unwrap();
        let requests = server.requests.lock().unwrap();
        let wire = requests
            .iter()
            .map(|r| serde_json::from_slice::<serde_json::Value>(&r.body).unwrap())
            .find(|v| fixture::pack_in(v).unwrap()["decision_id"] == intent["decision_id"])
            .unwrap();
        assert!(
            wire["text"]["format"]["schema"] == intent["schema"],
            "SDK schema must match the bound intent"
        );
        let request_bytes = requests
            .iter()
            .find(|r| {
                fixture::pack_in(&serde_json::from_slice(&r.body).unwrap()).unwrap()["decision_id"]
                    == intent["decision_id"]
            })
            .unwrap()
            .body
            .len();
        let usage: serde_json::Value = row.get(4);
        eprintln!(
            "safe_sdk_sizes case={slot} pack={} intent={} request_entity={} response_entity={} context_margin={} intent_margin={} request_margin={}",
            p.len(),
            i.len(),
            request_bytes,
            usage["response_bytes"],
            8192 - p.len(),
            16384 - i.len(),
            16384 - request_bytes
        );
    }
    std::fs::remove_file(path).unwrap();
    observed_packs
}

#[path = "support/m1_inference_lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;

#[test]
fn sdk_context_reaches_real_owner_pack_before_reservation() {
    let _guard = native::db();
    refresh();
    let q = registered(0x2a101);
    let record = run_support::start_only(&q);
    let server = fixture::Server::start("127.0.0.1:18083", |_| fixture::Reply::Reset);
    let before = owner_counts(record.decision_one);
    assert_eq!(before, [0, 0, 0]);
    // Existing helper crosses run lease/phase, owner views and real ContextPack.
    // It must succeed without inventing any future generation or call identity.
    let preparation_started = std::time::Instant::now();
    let prepared = lifecycle::prepare(&record, duskweave::m1_run::RunPhase::DecisionOne);
    eprintln!(
        "safe_sdk_preparation stage=owner_context_preflight elapsed_ms={} reservations=0 calls=0 sends={}",
        preparation_started.elapsed().as_millis(),
        server.count()
    );
    let p: serde_json::Value = serde_json::from_str(prepared.pack.utf8()).unwrap();
    assert_eq!(p["version"], "m1_context_pack_autonomous_v3");
    assert_eq!(owner_counts(record.decision_one), before);
    assert_eq!(
        server.count(),
        0,
        "real ContextPack preflight must not send"
    );
    capacity_boundaries(prepared.pack.utf8().as_bytes());
    use duskweave::m1_reasoning::{inference::Broker, inference_intent as intent};
    let mut broker = lifecycle::broker(&record, &prepared);
    let reserved = broker
        .reserve(&prepared.scope, prepared.pack.utf8().as_bytes())
        .unwrap();
    let upper = intent::preflight(prepared.pack.utf8().as_bytes(), 8192, 16384).unwrap();
    for (call, codes) in [
        (reserved.generation_call, None),
        (
            reserved.repair_call,
            Some(&["unexpected_view_request"; 8][..]),
        ),
    ] {
        let bound = intent::bind(
            &prepared.scope,
            &reserved,
            call,
            prepared.pack.utf8().as_bytes(),
            codes,
        )
        .unwrap();
        assert!(bound.configuration.len() <= upper);
        let i: serde_json::Value = serde_json::from_slice(&bound.configuration).unwrap();
        assert_eq!(i["generation"], reserved.generation);
        assert_eq!(
            i["generation_call_id"],
            reserved.generation_call.to_string()
        );
        assert_eq!(i["repair_call_id"], reserved.repair_call.to_string());
        assert_eq!(i["call_id"], call.to_string());
        if codes.is_none() {
            assert_rehashed_tampering_refused(&mut broker, &prepared.scope, &reserved, &bound);
        }
    }
    assert_eq!(
        server.count(),
        0,
        "binding actual reservation must not send"
    );
}
fn owner_counts(d: uuid::Uuid) -> [i64; 3] {
    let row=native::admin().query_one("SELECT (SELECT count(*) FROM execution.inference_reservations WHERE operation_id=$1), (SELECT count(*) FROM execution.inference_calls WHERE d=$1), (SELECT count(*) FROM execution.inference_calls WHERE d=$1 AND state='may_send_uncertain')",&[&d]).unwrap();
    [row.get(0), row.get(1), row.get(2)]
}
fn capacity_boundaries(pack: &[u8]) {
    use duskweave::m1_reasoning::{inference_intent as intent, request};
    let upper = intent::preflight(pack, pack.len(), 16384).unwrap();
    assert!(intent::preflight(pack, pack.len() - 1, 16384).is_err());
    assert_eq!(intent::preflight(pack, 8192, upper).unwrap(), upper);
    assert!(intent::preflight(pack, 8192, upper - 1).is_err());
    assert!(
        request::bind(pack, None).is_err(),
        "pre-reservation vendor binding stays refused"
    );
    let mut exact = pack.to_vec();
    exact.resize(8192, b' ');
    assert!(intent::preflight(&exact, 8192, 16384).is_ok());
    exact.push(b' ');
    assert!(intent::preflight(&exact, 8192, 16384).is_err());
    let original: serde_json::Value = serde_json::from_slice(pack).unwrap();
    for text in ["\"\\".repeat(200), "\u{754c}".repeat(200)] {
        let mut p = original.clone();
        p["question"] = serde_json::json!(text);
        assert!(intent::preflight(&serde_json::to_vec(&p).unwrap(), 8192, 16384).is_ok());
    }
    for (key, value) in [
        ("version", "m1_context_pack_autonomous_v2"),
        ("inference_profile", "fixture-responses-v1"),
        ("execution_mode", "live"),
    ] {
        let mut p = original.clone();
        p[key] = serde_json::json!(value);
        assert!(intent::preflight(&serde_json::to_vec(&p).unwrap(), 8192, 16384).is_err());
    }
}

fn assert_rehashed_tampering_refused(
    broker: &mut duskweave::m1_reasoning::postgres::PgBroker,
    scope: &duskweave::m1_reasoning::inference::Scope,
    reserved: &duskweave::m1_reasoning::inference::Reservation,
    original: &duskweave::m1_reasoning::request::BoundCall,
) {
    use duskweave::m1_reasoning::{
        inference::Broker, inference_intent as intent, request::BoundCall,
    };
    let before = owner_counts(scope.decision);
    for (field, value) in [
        ("recipient", "http://127.0.0.1:18084/responses"),
        ("model", "unapproved"),
        ("permission_id", "00000000-0000-0000-0000-000000000099"),
        ("campaign_id", "00000000-0000-0000-0000-000000000099"),
        ("call_role", "repair"),
        ("generation", "2"),
        ("settings", "changed"),
    ] {
        let mut i: serde_json::Value = serde_json::from_slice(&original.configuration).unwrap();
        match field {
            "settings" => i["settings"]["store"] = serde_json::json!(true),
            "generation" => i[field] = serde_json::json!(reserved.generation + 1),
            _ => i[field] = serde_json::json!(value),
        }
        i.sort_all_objects();
        let configuration = serde_json::to_vec(&i).unwrap();
        let tampered = BoundCall {
            pack: original.pack.clone(),
            body: Vec::new(),
            digest: intent::digest(&original.pack, &configuration),
            configuration,
        };
        assert!(
            intent::verify_owned(&tampered, scope, reserved, reserved.generation_call).is_err(),
            "Rust must refuse recomputed {field} tampering"
        );
        assert!(
            broker
                .may_send(scope, reserved.generation_call, &tampered)
                .is_err(),
            "restricted SQL must refuse recomputed {field} tampering"
        );
        assert_eq!(
            owner_counts(scope.decision),
            before,
            "invalid admission cannot retain a marker"
        );
    }
}

#[test]
fn sdk_cli_defer_repair_exhaustion_incomplete_protocol_and_unknown_never_replay() {
    let _guard = native::db();
    refresh();
    for (index, mode) in [
        "defer",
        "repair",
        "exhaust",
        "incomplete",
        "protocol",
        "unknown",
    ]
    .into_iter()
    .enumerate()
    {
        let mut sequence = 0;
        let server = fixture::Server::start("127.0.0.1:18083", move |r| {
            sequence += 1;
            if mode == "unknown" {
                return fixture::Reply::Reset;
            }
            let wire: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
            let pack = fixture::pack_in(&wire).unwrap();
            let mut proposal = fixture::proposal(&pack);
            if mode == "defer" {
                proposal["next_intent"] = serde_json::json!({"category":"defer","references":[],"manifest_reference":null});
            }
            if mode == "exhaust" || (mode == "repair" && sequence == 1) {
                proposal = serde_json::json!({});
            }
            let mut envelope = fixture::envelope(&proposal);
            if mode == "incomplete" {
                envelope["status"] = serde_json::json!("incomplete");
                envelope["incomplete_details"] = serde_json::json!({"reason":"max_output_tokens"});
            }
            if mode == "protocol" {
                envelope["usage"]["input_tokens"] =
                    serde_json::json!("synthetic-malformed-typed-usage");
            }
            fixture::Reply::Length(200, serde_json::to_vec(&envelope).unwrap())
        });
        let q = registered(0x2a200 + index as u128);
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
        let text = String::from_utf8_lossy(&output.stdout);
        let receipt: serde_json::Value =
            serde_json::from_str(text.lines().find(|l| l.starts_with('{')).unwrap()).unwrap();
        let (status, calls, effects) = match mode {
            "defer" => ("deferred", 1, 0),
            "repair" => ("completed", 3, 1),
            "exhaust" => ("unknown", 2, 0),
            _ => ("unknown", 1, 0),
        };
        assert_eq!(receipt["status"], status, "{mode}: {receipt}");
        assert_eq!(server.count(), calls, "{mode} single-send/repair bound");
        assert_eq!(receipt["effect_count"], effects, "{mode}");
        let disposition = receipt["recorded_decisions"][0]["disposition"]
            .as_str()
            .unwrap();
        let expected = match mode {
            "defer" | "repair" => "validated_proposal",
            "exhaust" => "schema_repair_exhausted",
            "incomplete" => "provider_incomplete",
            "protocol" => "provider_protocol",
            _ => "provider_transport_unknown",
        };
        assert_eq!(disposition, expected, "{mode}");
        for op in ["m1-run-inspect", "m1-run-recover", "m1-run-recover"] {
            let (_, r) = run_support::cli(op, "--run", q["run_id"].as_str().unwrap());
            assert_eq!(
                r["effect_count"], receipt["effect_count"],
                "{mode} recovery effects"
            );
            assert_eq!(
                r["decision_count"], receipt["decision_count"],
                "{mode} recovery identity"
            );
            assert_eq!(server.count(), calls, "{mode} recovery must never send");
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn sdk_stale_record_preserves_observe_intent_and_blocks_restricted_effect_release() {
    use duskweave::m1_reasoning::{
        inference, postgres::PgDecisionStore, sdk_transport::SdkProvider, transport_control::Permit,
    };
    use duskweave::m1_run::{NextStep, RunPhase};
    let _guard = native::db();
    refresh();
    let server = fixture::Server::start("127.0.0.1:18083", |r| {
        let p = fixture::pack_in(&serde_json::from_slice(&r.body).unwrap()).unwrap();
        fixture::Reply::Length(
            200,
            serde_json::to_vec(&fixture::envelope(&fixture::proposal(&p))).unwrap(),
        )
    });
    let q = registered(0x2a300);
    let record = run_support::start_only(&q);
    let lease = run_support::LeaseCustodian::start(&record, run_support::broker_store());
    let prepared = lifecycle::prepare(&record, RunPhase::DecisionOne);
    let mut broker = lifecycle::broker(&record, &prepared);
    let mut basis = lifecycle::ExpiredAtRecording(lifecycle::basis(&prepared, broker.deadline));
    let permit = Permit::new(broker.deadline);
    assert!(permit.publish(std::time::Instant::now(), broker.deadline));
    let mut provider = SdkProvider::new(permit, || Ok(()));
    let receipt = inference::infer(
        &mut broker,
        &mut provider,
        &mut basis,
        &mut PgDecisionStore {
            client: native::client("DW_COGNITION_DATABASE_URL"),
            input_hash: prepared.hash,
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
        store.first_step(&record).unwrap(),
        NextStep::Terminal("deferred")
    );
    store
        .complete_phase(record.request.run_id, RunPhase::DecisionOne)
        .unwrap();
    store
        .start_phase(record.request.run_id, RunPhase::Acquisition)
        .unwrap();
    store
        .claim_phase(
            record.request.run_id,
            RunPhase::Acquisition,
            &run_support::release(),
        )
        .unwrap();
    assert!(store.reserve_effect(record.request.run_id, 0).is_err());
    drop(provider);
    lease.finish().unwrap();
    store.terminal(record.request.run_id, "deferred").unwrap();
    assert_eq!(server.count(), 1);
    for op in ["m1-run-inspect", "m1-run-recover"] {
        let (_, r) = run_support::cli(op, "--run", q["run_id"].as_str().unwrap());
        assert_eq!(r["effect_count"], 0);
        assert_eq!(
            r["recorded_decisions"][0]["current_use"],
            "blocked_stale_premises"
        );
        assert_eq!(server.count(), 1);
    }
}

#[test]
fn sdk_stop_before_release_keeps_zero_calls_and_effects() {
    use duskweave::m1_run::RunPhase;
    let _guard = native::db();
    refresh();
    let server = fixture::Server::start("127.0.0.1:18083", |_| fixture::Reply::Reset);
    let q = registered(0x2a400);
    let record = run_support::start_only(&q);
    let mut store = run_support::broker_store();
    store
        .start_phase(record.request.run_id, RunPhase::DecisionOne)
        .unwrap();
    let (ok, _) = run_support::cli("m1-run-stop", "--run", q["run_id"].as_str().unwrap());
    assert!(ok);
    assert!(
        store
            .claim_phase(
                record.request.run_id,
                RunPhase::DecisionOne,
                &run_support::release()
            )
            .is_err()
    );
    for _ in 0..2 {
        let (_, r) = run_support::cli("m1-run-recover", "--run", q["run_id"].as_str().unwrap());
        assert_eq!(r["decision_count"], 0);
        assert_eq!(r["effect_count"], 0);
        assert_eq!(server.count(), 0);
    }
    assert_eq!(owner_counts(record.decision_one), [0, 0, 0]);
}
#[test]
fn sdk_observed_send_stop_lease_loss_and_parent_loss_terminate_without_replay() {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };
    let _guard = native::db();
    refresh();
    let selected = std::env::var("DW_TEST_SDK_FAULT_MODE").ok();
    assert!(
        selected
            .as_deref()
            .is_none_or(|m| ["stop", "lease", "parent"].contains(&m)),
        "closed test mode"
    );
    for (index, mode) in ["stop", "lease", "parent"].into_iter().enumerate() {
        if selected.as_deref().is_some_and(|m| m != mode) {
            continue;
        }
        let held = Arc::new(AtomicBool::new(false));
        let gate = held.clone();
        let server = fixture::Server::start("127.0.0.1:18083", move |_| {
            fixture::Reply::Hold(gate.clone())
        });
        let q = registered(0x2a410 + index as u128);
        let run = run_support::id(&q);
        let mut observer = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
        observer
            .batch_execute("SET statement_timeout='200ms'; SET lock_timeout='100ms'")
            .unwrap();
        let (mut parent, log, launched) = launch_sdk_run(&q);
        await_current_run_request(
            &server,
            &mut parent,
            &mut observer,
            &q,
            &log,
            launched,
            mode,
        );
        let (child, _) = parent.phase().expect("owned phase child before fault");
        assert!(
            parent.try_wait().unwrap().is_none()
                && !run_support::observe_child_exit(child, Instant::now()),
            "{mode}: intended post-send fault requires a live product child and parent"
        );
        let injected = Instant::now();
        match mode {
            "stop" => {
                let (ok, _) = run_support::cli("m1-run-stop", "--run", &run.to_string());
                assert!(ok);
            }
            "lease" => {
                native::admin()
                    .execute(
                        "UPDATE execution.autonomous_control SET lease_until_ms=0 WHERE run_id=$1",
                        &[&run],
                    )
                    .unwrap();
            }
            _ => {
                parent.kill().unwrap();
                parent.wait().unwrap();
            }
        }
        let until = Instant::now() + Duration::from_secs(8);
        let child_terminated = run_support::observe_child_exit(child, until);
        while parent.try_wait().unwrap().is_none() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        let parent_terminated = parent.try_wait().unwrap().is_some();
        eprintln!(
            "safe_sdk_fault mode={mode} child_terminated={child_terminated} parent_terminated={parent_terminated} elapsed_ms={}",
            injected.elapsed().as_millis()
        );
        if !parent_terminated {
            let _ = parent.kill();
            let _ = parent.wait();
        }
        held.store(true, Ordering::SeqCst);
        assert!(
            child_terminated && parent_terminated,
            "{mode}: bounded product termination required"
        );
        if mode == "parent" {
            std::thread::sleep(Duration::from_millis(2100));
        }
        let (_, before) = run_support::cli("m1-run-inspect", "--run", &run.to_string());
        assert_eq!(before["effect_count"], 0);
        assert_eq!(server.count(), 1);
        assert_eq!(
            before["inference"].as_array().unwrap().len(),
            1,
            "possible send remains charged"
        );
        for _ in 0..2 {
            let (_, after) = run_support::cli("m1-run-recover", "--run", &run.to_string());
            assert_eq!(after["effect_count"], 0);
            assert_eq!(server.count(), 1, "{mode} cannot replay");
            assert_eq!(after["decision_one"], before["decision_one"]);
            assert_eq!(
                after["inference"], before["inference"],
                "{mode} retains original charge/usage uncertainty"
            );
        }
    }
}

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

#[test]
fn sdk_d2_omits_optional_summary_under_admitted_cap_before_removing_candidates() {
    let packs = assert_sdk_real_cli(0x2a103, |raw| {
        wire::eight_seeds(raw);
        raw["m1_inference_permission"]["operation_limits"]["context_bytes"] =
            serde_json::json!(7520);
    });
    let feedback = packs
        .iter()
        .find(|p| p["predecessor"]["availability"] == "committed")
        .unwrap();
    assert_eq!(
        feedback["candidates"].as_array().unwrap().len(),
        8,
        "omit optional summary before discarding admitted candidates"
    );
    assert!(feedback["predecessor"].get("hypothesis_summary").is_none());
    assert_eq!(
        feedback["predecessor"]["hypothesis_summary_omission"],
        "byte_bound"
    );
    assert_eq!(
        feedback["predecessor"]["outcome"],
        "owner_qualified_observed"
    );
    assert!(feedback["predecessor"]["terrain_publication_ref"].is_string());
    assert!(
        feedback["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == "previous_decision")
    );
}

fn safe_parent_terminal(log: &std::path::Path, mode: &str) {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let receipt = text
        .lines()
        .find_map(|l| serde_json::from_str::<serde_json::Value>(l).ok());
    let status = match receipt.as_ref().and_then(|v| v["status"].as_str()) {
        Some("unknown") => "unknown",
        Some("stopped") => "stopped",
        Some("completed") => "completed",
        _ => "unavailable",
    };
    let diagnostic = receipt.as_ref().map(|v| &v["diagnostics"][0]);
    let ready = diagnostic
        .and_then(|d| d["readiness_observed"].as_bool())
        .unwrap_or(false);
    let cause = match diagnostic.and_then(|d| d["cause"].as_str()) {
        Some("readiness_deadline") => "readiness_deadline",
        Some("child_error") => "child_error",
        Some("phase_deadline") => "phase_deadline",
        _ => "unavailable",
    };
    let error = match diagnostic.and_then(|d| d["child_error"].as_str()) {
        Some("run_inference_pending") => "run_inference_pending",
        Some("inference_control_unavailable") => "inference_control_unavailable",
        _ => "unavailable",
    };
    let disposition = match receipt
        .as_ref()
        .and_then(|v| v["recorded_decisions"][0]["disposition"].as_str())
    {
        Some("inference_deadline") => "inference_deadline",
        Some("provider_cancelled_uncertain") => "provider_cancelled_uncertain",
        Some("provider_transport_unknown") => "provider_transport_unknown",
        _ => "unavailable",
    };
    eprintln!(
        "safe_sdk_parent mode={mode} status={status} ready={ready} cause={cause} error={error} disposition={disposition}"
    );
}
fn await_current_run_request(
    server: &fixture::Server,
    parent: &mut run_support::OwnedRun,
    observer: &mut postgres::Client,
    q: &serde_json::Value,
    log: &std::path::Path,
    launched: std::time::Instant,
    mode: &str,
) {
    use std::time::{Duration, Instant};
    let run = run_support::id(q);
    // The request still belongs to the original absolute run and phase bounds.
    // Observing readiness or a reservation never creates a new clock origin.
    let wall = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let remaining = q["deadline_at_ms"].as_i64().unwrap() - wall;
    assert!((1..=400000).contains(&remaining), "original run bound");
    let mut end = Instant::now() + Duration::from_millis(remaining as u64);
    let mut seen = [false; 7];
    loop {
        if let Some(bounds) = run_support::phase_bounds(
            observer,
            run,
            duskweave::m1_inference_profile::REASONING_BOUNDED.phase_ms,
        ) {
            end = end.min(bounds.earliest_stop);
            for (index, at) in [(0, Some(bounds.started)), (1, bounds.claimed)] {
                if let Some(at) = at
                    && !seen[index]
                {
                    seen[index] = true;
                    eprintln!(
                        "safe_sdk_startup mode={mode} stage={} elapsed_ms={} uncertainty_ms={}",
                        ["phase_start", "phase_claim"][index],
                        at.saturating_duration_since(launched).as_millis(),
                        bounds.uncertainty.as_millis()
                    );
                }
            }
            if bounds.claimed.is_some() {
                let _ = parent.phase();
            }
        }
        let row=observer.query_opt("SELECT x.decision_one, (SELECT count(*) FROM execution.inference_reservations a WHERE a.operation_id=x.decision_one), (SELECT count(*) FROM execution.inference_calls a WHERE a.d=x.decision_one AND a.state='may_send_uncertain'), EXISTS(SELECT 1 FROM pg_stat_activity a WHERE a.datname=current_database() AND a.usename=session_user AND a.pid<>pg_backend_pid() AND a.state='idle' AND a.query='SELECT execution.inference_window_ms($1,$2,$3)'), EXISTS(SELECT 1 FROM pg_stat_activity a WHERE a.datname=current_database() AND a.usename=session_user AND a.pid<>pg_backend_pid() AND position('mission.current_inference_publication' in a.query)>0), EXISTS(SELECT 1 FROM pg_stat_activity a WHERE a.datname=current_database() AND a.usename=session_user AND a.pid<>pg_backend_pid() AND position('inference_reservation(' in a.query)>0) FROM execution.autonomous_runs x WHERE x.run_id=$1",&[&run]).unwrap_or_else(|_|panic!("restricted startup observation unavailable"));
        if let Some(row) = row {
            let flags = [
                row.get::<_, bool>(3),
                row.get::<_, bool>(4),
                row.get::<_, bool>(5),
                row.get::<_, i64>(1) > 0,
                row.get::<_, i64>(2) > 0,
            ];
            for (i, flag) in flags.into_iter().enumerate() {
                if flag && !seen[i + 2] {
                    seen[i + 2] = true;
                    if i >= 2 {
                        eprintln!(
                            "safe_sdk_startup mode={mode} stage=context_preflight_and_readiness_completed upper_bound_ms={}",
                            launched.elapsed().as_millis()
                        );
                    }
                    eprintln!(
                        "safe_sdk_startup mode={mode} stage={} observed_ms={}",
                        [
                            "window_acquired",
                            "context_read",
                            "ready_before_reservation",
                            "reservation_committed",
                            "may_send"
                        ][i],
                        launched.elapsed().as_millis()
                    );
                }
            }
            if server.count() > 0 {
                assert_eq!(
                    server.count(),
                    1,
                    "exactly one complete request before fault: {mode}"
                );
                let requests = server.requests.lock().unwrap();
                let wire: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
                let p = fixture::pack_in(&wire).expect("immutable current-run pack");
                assert!(
                    p["decision_id"] == row.get::<_, uuid::Uuid>(0).to_string(),
                    "complete request must belong to the current decision"
                );
                for key in ["engagement_id", "campaign_id", "permission_id"] {
                    assert!(
                        p[key] == q["context"]["mission"][key],
                        "complete request must belong to the current run scope"
                    );
                }
                eprintln!(
                    "safe_sdk_startup mode={mode} stage=actual_send elapsed_ms={} requests=1",
                    launched.elapsed().as_millis()
                );
                return;
            }
        }
        if parent.try_wait().unwrap().is_some() {
            safe_parent_terminal(log, mode);
            panic!("parent terminated before intended send: {mode}");
        }
        if Instant::now() >= end {
            safe_parent_terminal(log, mode);
            eprintln!(
                "safe_sdk_startup mode={mode} stage=send_barrier elapsed_ms={} requests={}",
                launched.elapsed().as_millis(),
                server.count()
            );
            panic!("intended send barrier not reached: {mode}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn launch_sdk_run(
    q: &serde_json::Value,
) -> (
    run_support::OwnedRun,
    std::path::PathBuf,
    std::time::Instant,
) {
    use std::process::Stdio;
    let run = run_support::id(q);
    let input = run_support::path(q);
    let root = run_support::control_root();
    let log = std::env::temp_dir().join(format!("dw-sdk-parent-{run}.log"));
    let output = std::fs::File::create(&log).unwrap();
    let launched = std::time::Instant::now();
    let child = run_support::command()
        .env("DW_M1_RUN_CONTROL_ROOT", &root)
        .args([
            "m1-run",
            "--input",
            input.to_str().unwrap(),
            "--timing-profile",
            "reasoning-bounded-v1",
        ])
        .stderr(Stdio::from(output.try_clone().unwrap()))
        .stdout(Stdio::from(output))
        .spawn()
        .unwrap();
    let mut owned = run_support::OwnedRun::new(child, run);
    owned.own_file(&input);
    owned.own_file(&log);
    owned.own_control_root(&root);
    (owned, log, launched)
}
#[test]
fn sdk_post_send_assertion_cleanup_settles_owned_processes_response_and_files() {
    let _guard = native::db();
    refresh();
    let held = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let gate = held.clone();
    let server = fixture::Server::start("127.0.0.1:18083", move |_| {
        fixture::Reply::Hold(gate.clone())
    });
    let q = registered(0x2a420);
    let mut observer = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let (mut parent, log, launched) = launch_sdk_run(&q);
    await_current_run_request(
        &server,
        &mut parent,
        &mut observer,
        &q,
        &log,
        launched,
        "cleanup",
    );
    let child = parent
        .phase()
        .expect("owned phase before cleanup injection")
        .0;
    let verdict = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _server = server;
        let _parent = parent;
        panic!("synthetic post-send assertion failure");
    }));
    assert!(verdict.is_err());
    // A custody verdict, deliberately distinct from product termination.
    assert!(run_support::observe_child_exit(
        child,
        std::time::Instant::now() + std::time::Duration::from_secs(2)
    ));
    assert!(!log.exists());
    assert!(
        std::net::TcpListener::bind("127.0.0.1:18083").is_ok(),
        "held response port must be reusable after failure"
    );
    let idle:i64=observer.query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND usename=session_user AND state='idle in transaction'",&[]).unwrap().get(0);
    assert_eq!(
        idle, 0,
        "owned SQL transaction must be settled before next case"
    );
}
