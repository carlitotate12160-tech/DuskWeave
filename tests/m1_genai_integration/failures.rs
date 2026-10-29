//! SDK failures qualification through the existing native fixture.
use super::*;

pub(super) fn sdk_cli_defer_repair_exhaustion_incomplete_protocol_and_unknown_never_replay() {
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

pub(super) fn sdk_stale_record_preserves_observe_intent_and_blocks_restricted_effect_release() {
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
