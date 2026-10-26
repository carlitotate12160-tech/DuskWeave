//! Focused PostgreSQL regression tests for DNS selector contract across 0017 and 0021.
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_terrain_context.rs"]
mod terrain;
use terrain::terrain_support::native;
#[allow(dead_code)]
#[path = "support/m1_inference_lifecycle.rs"]
mod lifecycle;
#[path = "support/m1_run.rs"]
mod run_support;

use duskweave::m1_reasoning::{
    inference::{self, Provider},
    proposal::{self, Admission, ProposalBasis},
    request,
};
use duskweave::m1_run::{RunPhase, RunRecord};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

struct SelectorProvider {
    refs: Vec<String>,
    intent_refs: Vec<String>,
    category: &'static str,
    manifest: Option<&'static str>,
    view_owner: Option<&'static str>,
}
impl Provider for SelectorProvider {
    fn prepare(&mut self, b: &request::BoundCall, _: Instant) -> Result<(), &'static str> {
        request::verify_binding(
            std::str::from_utf8(&b.pack).unwrap(),
            std::str::from_utf8(&b.configuration).unwrap(),
            std::str::from_utf8(&b.body).unwrap(),
            &b.digest,
            false,
        )
    }
    fn call(
        &mut self,
        b: &request::BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<inference::Received, &'static str> {
        release()?;
        let mut intent = json!({"category":self.category,"references":self.intent_refs,"manifest_reference":self.manifest});
        if self.category == "observe_more" {
            intent["view_request"] = self.view_owner.map_or(
                Value::Null,
                |o| json!({"owner":o,"selectors":["mission","asset_0"]}),
            );
        }
        let p = json!({"version":"m1_inference_proposal_v1","hypothesis":"Objective-led routing hypothesis.",
            "considered_alternative":"Defer if routing cannot be observed.","references":self.refs,
            "expected_observation":"One bounded fixture DNS observation.","falsifying_observation":"Missing routing response.",
            "uncertainty":"Controlled fixture only.","rationale":"Context supports one selected asset.","next_intent":intent});
        let pack: Value = serde_json::from_slice(&b.pack).unwrap();
        if pack["version"] == "m1_context_pack_autonomous_v1" {
            let raw = serde_json::to_vec(&p).unwrap();
            return Ok(inference::Received {
                admission: proposal::admit(&raw, basis),
                accounting_confirmed: false,
                usage: inference::Usage {
                    input_tokens: None,
                    output_tokens: None,
                    reasoning_tokens: None,
                    response_bytes: raw.len(),
                    wall_time_ms: 0,
                    requested_model: "deterministic-autonomous-v1",
                    reported_model: Some("deterministic-autonomous-v1".into()),
                },
            });
        }
        let chat = serde_json::from_slice::<Value>(&b.configuration).unwrap()["protocol"]
            == "chat-completions-v1";
        let env = if chat {
            json!({"choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":p.to_string()}}]})
        } else {
            json!({"status":"completed","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":p.to_string()}]}]})
        };
        let raw = serde_json::to_vec(&env).unwrap();
        let mut received = if chat {
            duskweave::m1_reasoning::chat_completions::admit_envelope(&raw, Duration::ZERO, basis)?
        } else {
            duskweave::m1_reasoning::response::admit_envelope(
                &raw,
                raw.len(),
                Duration::ZERO,
                basis,
            )?
        };
        received.usage.requested_model = "protocol-fixture-v1";
        Ok(received)
    }
}

fn two_seeds(raw: &mut Value) {
    let rule = raw["m1_permission"]["contact_rules"][0].clone();
    let assets = [Uuid::from_u128(0x21), Uuid::from_u128(0x101)];
    raw["included_assets"] = json!(assets);
    let mut second = rule.clone();
    second["asset_ref"] = json!(assets[1]);
    second["priority"] = json!(2);
    second["rule"] = json!({"exact":"seed1.example.invalid"});
    raw["m1_permission"]["contact_rules"] = json!([rule, second]);
}

fn three_seeds(context_bytes: usize) -> impl Fn(&mut Value) {
    move |raw| {
        let rule = raw["m1_permission"]["contact_rules"][0].clone();
        let assets = [
            Uuid::from_u128(0x21),
            Uuid::from_u128(0x101),
            Uuid::from_u128(0x102),
        ];
        raw["included_assets"] = json!(assets);
        raw["m1_permission"]["contact_rules"] = json!(
            assets
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let mut rule = rule.clone();
                    rule["asset_ref"] = json!(id);
                    rule["priority"] = json!(i + 1);
                    rule["rule"] = json!({"exact":format!("seed{i}.example.invalid")});
                    rule
                })
                .collect::<Vec<_>>()
        );
        raw["m1_inference_permission"]["operation_limits"]["context_bytes"] = json!(context_bytes);
    }
}

fn three_seed_registration(slot: u128, successor: bool, context_bytes: usize) -> Value {
    if successor {
        lifecycle::registered_with(slot, "fixture-responses-v1", three_seeds(context_bytes))
    } else {
        run_support::registered_with(slot, three_seeds(context_bytes))
    }
}

fn reduced_setup(
    slot: u128,
    successor: bool,
    context_bytes: usize,
) -> (Value, RunRecord, run_support::LeaseCustodian) {
    if successor {
        run_support::refresh();
    } else {
        run_support::refresh_legacy();
    }
    let q = three_seed_registration(slot, successor, context_bytes);
    let rec = run_support::start_only(&q);
    let lease = run_support::LeaseCustodian::start(&rec, run_support::broker_store());
    (q, rec, lease)
}

// Construct the reduced ContextPack artifact through the real producer: the
// context's candidate omission drops the middle seed while its asset_UUID
// identity is preserved, so pack-local asset_1 still names original
// seed_names[2]. No byte-pressure machinery is built here; the reduced
// artifact is bound and recorded through the existing inference path.
fn prepare_reduced_pack(rec: &RunRecord) -> lifecycle::Prepared {
    use sha2::{Digest, Sha256};
    let mut run = run_support::broker_store();
    let r = rec.request.run_id;
    run.renew(r).unwrap();
    run.start_phase(r, RunPhase::DecisionOne).unwrap();
    run.claim_phase(r, RunPhase::DecisionOne, &run_support::release())
        .unwrap();
    let mut qv = serde_json::to_value(&rec.request.context).unwrap();
    qv["candidates"].as_array_mut().unwrap().remove(1);
    let q: duskweave::m1_context_input::ContextRequest = serde_json::from_value(qv).unwrap();
    let d = rec.decision_one;
    let hash = Sha256::digest(serde_json::to_vec(&q).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let scope = inference::Scope {
        engagement: q.mission.engagement_id.0,
        campaign: q.mission.campaign_id.0,
        decision: d,
        mission_revision: 1,
        permission: q.mission.permission_id,
    };
    native::client("DW_INFERENCE_BROKER_DATABASE_URL")
        .execute(
            "SELECT execution.bind_inference_input($1,$2,$3,$4)",
            &[&scope.engagement, &scope.campaign, &d, &hash],
        )
        .unwrap();
    native::client("DW_INFERENCE_BROKER_DATABASE_URL")
        .query_one(
            "SELECT execution.inference_window_ms($1,$2,$3)",
            &[&scope.engagement, &scope.campaign, &scope.decision],
        )
        .unwrap();
    let a = duskweave::m1_autonomous_context::AutonomousContext {
        binding: duskweave::m1_autonomous_context::RunContextBinding {
            engagement_id: q.mission.engagement_id,
            campaign_id: q.mission.campaign_id,
            run_profile: rec.request.run_profile.clone(),
            execution_mode: rec.request.execution_mode.clone(),
            inference_profile: rec.request.inference_profile.clone(),
            release_sha256: rec.release_sha256.clone(),
            qualification_ref: rec.qualification_ref,
        },
        predecessor: None,
    };
    let pack = duskweave::m1_context::prepare_autonomous(
        &mut duskweave::postgres_mission::PgContextReader::new(native::client(
            "DW_INFERENCE_BROKER_DATABASE_URL",
        )),
        &q,
        duskweave::mission::OperationId(d),
        &a,
    )
    .unwrap();
    lifecycle::Prepared { scope, pack, hash }
}

fn setup_run(slot: u128, successor: bool) -> (Value, RunRecord, run_support::LeaseCustodian) {
    if successor {
        run_support::refresh();
    } else {
        run_support::refresh_legacy();
    }
    let q = if successor {
        lifecycle::registered_with(slot, "fixture-responses-v1", two_seeds)
    } else {
        run_support::registered_with(slot, two_seeds)
    };
    let rec = run_support::start_only(&q);
    let lease = run_support::LeaseCustodian::start(&rec, run_support::broker_store());
    (q, rec, lease)
}

#[test]
fn valid_selector_cases_reserve_expected_seed_across_0017_and_0021_without_rewriting_proposal() {
    let _guard = native::db();
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/m1-dns-selection-cases.json")).unwrap();
    for (mode_idx, successor) in [false, true].into_iter().enumerate() {
        for (case_idx, case) in fixture["valid"].as_array().unwrap().iter().enumerate() {
            let name = case["name"].as_str().unwrap();
            let refs: Vec<String> = serde_json::from_value(case["references"].clone()).unwrap();
            let expected = case["expected_seed"].as_u64().unwrap() as usize;
            let (_q, rec, lease) =
                setup_run(0xd100 + (mode_idx * 0x100 + case_idx) as u128, successor);
            let prepared = lifecycle::prepare(&rec, RunPhase::DecisionOne);
            let mut provider = SelectorProvider {
                refs: vec!["mission".into(), format!("asset_{expected}")],
                intent_refs: refs.clone(),
                category: "observe_more",
                manifest: Some("dns_udp"),
                view_owner: None,
            };
            let receipt = lifecycle::infer(&rec, &prepared, &mut provider);
            assert_eq!(receipt.proposal_disposition, "validated_proposal", "{name}");
            assert_eq!(receipt.history, "completed", "{name}");
            let mut store = run_support::broker_store();
            let r = rec.request.run_id;
            store.complete_phase(r, RunPhase::DecisionOne).unwrap();
            store.start_phase(r, RunPhase::Acquisition).unwrap();
            store
                .claim_phase(r, RunPhase::Acquisition, &run_support::release())
                .unwrap();
            assert!(
                store.reserve_effect(r, 1 - expected).is_err(),
                "{name}: seed substitution"
            );
            let issued = store
                .reserve_effect(r, expected)
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            assert_eq!(
                issued.subject_name, rec.request.seed_names[expected].name,
                "{name}"
            );
            assert_eq!(
                issued.asset_ref,
                Some(rec.request.seed_names[expected].asset_ref),
                "{name}"
            );
            assert!(
                store.reserve_effect(r, expected).is_err(),
                "{name}: duplicate reservation"
            );
            assert_eq!(
                store.first_proposal(&rec).unwrap().next_intent.references,
                refs,
                "{name}"
            );
            let history_refs: Value = native::client("DW_TRAJECTORY_DATABASE_URL")
                .query_one(
                    "SELECT contract#>'{proposal,next_intent,references}' FROM trajectory.reasoning_history WHERE operation_id=$1 AND status='accepted'",
                    &[&rec.decision_one],
                )
                .unwrap()
                .get(0);
            assert_eq!(history_refs, json!(refs), "{name}");
            lease.finish().unwrap();
        }
    }
    run_support::refresh();
}

#[test]
fn rejected_cases_and_unissued_references_fail_in_admission_and_direct_sql_across_0017_and_0021() {
    let _guard = native::db();
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/m1-dns-selection-cases.json")).unwrap();
    for (mode_idx, successor) in [false, true].into_iter().enumerate() {
        let (_q, rec, lease) = setup_run(0xd300 + mode_idx as u128, successor);
        let prepared = lifecycle::prepare(&rec, RunPhase::DecisionOne);
        let mut basis = lifecycle::basis(&prepared, Instant::now() + Duration::from_secs(20));
        for unknown in [
            vec!["previous_decision", "asset_0"],
            vec!["terrain_0", "asset_0"],
            vec!["forged_context", "asset_0"],
            vec!["asset_2"],
            vec!["asset_00"],
            vec!["seed0.example.invalid", "asset_0"],
        ] {
            let raw = serde_json::to_vec(&json!({"version":"m1_inference_proposal_v1","hypothesis":"h",
                "considered_alternative":"a","references":["mission","asset_0"],"expected_observation":"e",
                "falsifying_observation":"f","uncertainty":"u","rationale":"r",
                "next_intent":{"category":"observe_more","references":unknown,"manifest_reference":"dns_udp","view_request":null}})).unwrap();
            assert!(matches!(
                proposal::admit(&raw, &mut basis),
                Admission::Rejected("reference_ineligible")
            ));
        }
        let mut provider = SelectorProvider {
            refs: vec!["mission".into(), "asset_0".into()],
            intent_refs: vec!["mission".into(), "asset_0".into()],
            category: "observe_more",
            manifest: Some("dns_udp"),
            view_owner: None,
        };
        assert_eq!(
            lifecycle::infer(&rec, &prepared, &mut provider).history,
            "completed"
        );
        let mut store = run_support::broker_store();
        let r = rec.request.run_id;
        store.complete_phase(r, RunPhase::DecisionOne).unwrap();
        store.start_phase(r, RunPhase::Acquisition).unwrap();
        store
            .claim_phase(r, RunPhase::Acquisition, &run_support::release())
            .unwrap();
        let mut extra: Vec<Value> = fixture["rejected"].as_array().unwrap().clone();
        extra.push(json!({"name":"unissued_previous_decision","references":["previous_decision","asset_0"]}));
        extra.push(json!({"name":"unissued_terrain_0","references":["terrain_0","asset_0"]}));
        let mut admin = native::admin();
        let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
        for case in &extra {
            let name = case["name"].as_str().unwrap();
            admin
                .execute(
                    "UPDATE cognition.reasoning_decisions SET payload=jsonb_set(payload,'{proposal,next_intent,references}',$1::jsonb) WHERE d=$2",
                    &[&case["references"], &rec.decision_one],
                )
                .unwrap();
            for seed in [-1i32, 0, 1, 2] {
                let mut tx = broker.transaction().unwrap();
                assert!(
                    tx.query_one("SELECT execution.reserve_fixture_dns($1,$2)", &[&r, &seed])
                        .is_err(),
                    "{name}"
                );
                tx.rollback().unwrap();
            }
        }
        admin
            .execute(
                "UPDATE cognition.reasoning_decisions SET payload=jsonb_set(jsonb_set(payload,'{proposal,next_intent,references}','[\"mission\",\"asset_0\"]'::jsonb),'{proposal,references}','[\"forged_top_ref\"]'::jsonb) WHERE d=$1",
                &[&rec.decision_one],
            )
            .unwrap();
        let mut tx = broker.transaction().unwrap();
        assert!(
            tx.query_one("SELECT execution.reserve_fixture_dns($1,0)", &[&r])
                .is_err()
        );
        tx.rollback().unwrap();
        let attempts: i64 = admin
            .query_one(
                "SELECT count(*) FROM execution.fixture_dns_attempts WHERE run_id=$1",
                &[&r],
            )
            .unwrap()
            .get(0);
        assert_eq!(attempts, 0);
        lease.finish().unwrap();
    }
    run_support::refresh();
}

#[test]
fn terminal_dispositions_and_pre_closure_or_withdrawn_guards_hold_with_contextual_references() {
    let _guard = native::db();
    run_support::refresh();
    for (idx, (category, manifest, view_owner, expected_terminal)) in [
        (
            "observe_more",
            None,
            Some("cyber_terrain"),
            "owner_view_requested",
        ),
        ("observe_more", None, None, "capability_gap"),
        ("retain", None, None, "retained"),
        ("defer", None, None, "deferred"),
        ("stop", None, None, "stopped"),
    ]
    .into_iter()
    .enumerate()
    {
        let (_q, rec, lease) = setup_run(0xd400 + idx as u128, true);
        let prepared = lifecycle::prepare(&rec, RunPhase::DecisionOne);
        let mut provider = SelectorProvider {
            refs: vec!["mission".into(), "asset_0".into()],
            intent_refs: vec!["mission".into(), "asset_0".into()],
            category,
            manifest,
            view_owner,
        };
        assert_eq!(
            lifecycle::infer(&rec, &prepared, &mut provider).history,
            "completed"
        );
        let mut store = run_support::broker_store();
        let r = rec.request.run_id;
        assert_eq!(
            store.first_step(&rec).unwrap(),
            duskweave::m1_run::NextStep::Terminal(expected_terminal)
        );
        store.complete_phase(r, RunPhase::DecisionOne).unwrap();
        lease.finish().unwrap();
        store.terminal(r, expected_terminal).unwrap();
        let summary = store.summary(r).unwrap();
        assert_eq!(summary["status"], expected_terminal);
        assert_eq!(summary["effect_count"], 0);
    }
    let (_q, rec, lease) = setup_run(0xd450, true);
    let prepared = lifecycle::prepare(&rec, RunPhase::DecisionOne);
    let mut provider = SelectorProvider {
        refs: vec!["mission".into(), "asset_0".into()],
        intent_refs: vec!["mission".into(), "asset_0".into()],
        category: "observe_more",
        manifest: Some("dns_udp"),
        view_owner: None,
    };
    assert_eq!(
        lifecycle::infer(&rec, &prepared, &mut provider).history,
        "completed"
    );
    let mut store = run_support::broker_store();
    let r = rec.request.run_id;
    assert!(store.reserve_effect(r, 0).is_err());
    store.complete_phase(r, RunPhase::DecisionOne).unwrap();
    store.start_phase(r, RunPhase::Acquisition).unwrap();
    store
        .claim_phase(r, RunPhase::Acquisition, &run_support::release())
        .unwrap();
    native::admin()
        .execute(
            "DELETE FROM execution.inference_closures WHERE d=$1",
            &[&rec.decision_one],
        )
        .unwrap();
    assert!(store.reserve_effect(r, 0).is_err());
    lease.finish().unwrap();
}

#[test]
fn reduced_pack_asset_ordinal_resolves_through_entity_to_original_seed_across_0017_and_0021() {
    let _guard = native::db();
    for (mode_idx, successor) in [false, true].into_iter().enumerate() {
        let slot = 0xd500 + (mode_idx * 0x40) as u128;
        let (_q, rec, lease) = reduced_setup(slot, successor, 8192);
        let prepared = prepare_reduced_pack(&rec);
        let pack: Value = serde_json::from_str(prepared.pack.utf8()).unwrap();
        let refs = pack["references"].as_array().unwrap();
        let entity = |id: &str| {
            refs.iter()
                .find(|r| r["id"] == id)
                .and_then(|r| r["entity"].as_str())
                .unwrap_or_default()
                .to_owned()
        };
        assert_eq!(entity("asset_0"), Uuid::from_u128(0x21).to_string());
        assert_eq!(entity("asset_1"), Uuid::from_u128(0x102).to_string());
        assert!(refs.iter().all(|r| r["id"] != "asset_2"));
        assert_eq!(pack["candidates"].as_array().unwrap().len(), 2);
        assert_eq!(rec.request.seed_names.len(), 3);

        let mut provider = SelectorProvider {
            refs: vec!["mission".into(), "asset_1".into()],
            intent_refs: vec!["mission".into(), "asset_1".into()],
            category: "observe_more",
            manifest: Some("dns_udp"),
            view_owner: None,
        };
        let receipt = lifecycle::infer(&rec, &prepared, &mut provider);
        assert_eq!(
            receipt.proposal_disposition, "validated_proposal",
            "successor={successor}"
        );
        assert_eq!(receipt.history, "completed", "successor={successor}");

        let mut store = run_support::broker_store();
        let r = rec.request.run_id;
        assert_eq!(
            store.first_step(&rec).unwrap(),
            duskweave::m1_run::NextStep::Dns { seed_index: 1 },
            "successor={successor}: Rust selects pack-local asset_1"
        );
        store.complete_phase(r, RunPhase::DecisionOne).unwrap();
        store.start_phase(r, RunPhase::Acquisition).unwrap();
        store
            .claim_phase(r, RunPhase::Acquisition, &run_support::release())
            .unwrap();

        // A proposal naming two distinct assets never reserves an effect.
        let mut admin = native::admin();
        admin
            .execute(
                "UPDATE cognition.reasoning_decisions SET payload=jsonb_set(payload,'{proposal,next_intent,references}','[\"asset_0\",\"asset_1\"]'::jsonb) WHERE d=$1",
                &[&rec.decision_one],
            )
            .unwrap();
        let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
        for seed in [-1i32, 0, 1, 2] {
            let mut tx = broker.transaction().unwrap();
            assert!(
                tx.query_one("SELECT execution.reserve_fixture_dns($1,$2)", &[&r, &seed])
                    .is_err(),
                "successor={successor}: two-asset proposal seed {seed}"
            );
            tx.rollback().unwrap();
        }
        admin
            .execute(
                "UPDATE cognition.reasoning_decisions SET payload=jsonb_set(payload,'{proposal,next_intent,references}','[\"mission\",\"asset_1\"]'::jsonb) WHERE d=$1",
                &[&rec.decision_one],
            )
            .unwrap();
        let attempts: i64 = admin
            .query_one(
                "SELECT count(*) FROM execution.fixture_dns_attempts WHERE run_id=$1",
                &[&r],
            )
            .unwrap()
            .get(0);
        assert_eq!(attempts, 0, "successor={successor}: invalid cases");

        // Pack-local asset_1 (entity of original seed 2) reserves exactly once.
        assert!(
            store.reserve_effect(r, 0).is_err(),
            "successor={successor}: asset_0 substitution"
        );
        assert!(
            store.reserve_effect(r, 2).is_err(),
            "successor={successor}: unissued asset_2 substitution"
        );
        let issued = store
            .reserve_effect(r, 1)
            .unwrap_or_else(|e| panic!("successor={successor}: pack-local asset_1: {e:?}"));
        assert_eq!(
            issued.subject_name, rec.request.seed_names[2].name,
            "successor={successor}: DNS subject belongs to original seed 2"
        );
        assert_eq!(
            issued.asset_ref,
            Some(rec.request.seed_names[2].asset_ref),
            "successor={successor}"
        );
        assert_ne!(
            issued.asset_ref,
            Some(rec.request.seed_names[1].asset_ref),
            "successor={successor}: not seed 1"
        );
        let attempt_seed: i32 = admin
            .query_one(
                "SELECT seed_index FROM execution.fixture_dns_attempts WHERE run_id=$1",
                &[&r],
            )
            .unwrap()
            .get(0);
        assert_eq!(
            attempt_seed, 2,
            "successor={successor}: stored seed_index is the original selected seed"
        );
        assert!(
            store.reserve_effect(r, 1).is_err(),
            "successor={successor}: repeat reservation"
        );
        let attempts: i64 = admin
            .query_one(
                "SELECT count(*) FROM execution.fixture_dns_attempts WHERE run_id=$1",
                &[&r],
            )
            .unwrap()
            .get(0);
        assert_eq!(attempts, 1, "successor={successor}");
        let results: i64 = admin
            .query_one(
                "SELECT count(*) FROM execution.fixture_dns_results WHERE run_id=$1",
                &[&r],
            )
            .unwrap()
            .get(0);
        assert_eq!(results, 0, "successor={successor}");

        // Proposal content remains unchanged in durable inspection.
        assert_eq!(
            store.first_proposal(&rec).unwrap().next_intent.references,
            ["mission", "asset_1"],
            "successor={successor}"
        );
        let history_refs: Value = native::client("DW_TRAJECTORY_DATABASE_URL")
            .query_one(
                "SELECT contract#>'{proposal,next_intent,references}' FROM trajectory.reasoning_history WHERE operation_id=$1 AND status='accepted'",
                &[&rec.decision_one],
            )
            .unwrap()
            .get(0);
        assert_eq!(
            history_refs,
            json!(["mission", "asset_1"]),
            "successor={successor}"
        );
        lease.finish().unwrap();
    }
    run_support::refresh();
}
