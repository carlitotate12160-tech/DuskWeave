//! Pro selection through real Mission, immutable intent and restricted accounting.
use super::fixture::*;
use super::run_support::*;
use super::successor_episode::{cli, command, control_root};
use duskweave::m1_reasoning::{inference::*, inference_intent, proposal};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;
const PRO: &str = "deepseek-genai-v4-pro-low-v1";
const MODEL: &str = "deepseek-v4-pro";

fn register(slot: u128, profile: &str) -> (Uuid, Uuid, Uuid) {
    register_shaped(slot, |raw| {
        raw["m1_inference_permission"]["profile"] = json!(profile);
        raw["m1_inference_permission"]["recipient"] = json!("https://api.deepseek.com/responses");
    })
}
fn usage(model: &'static str) -> Usage {
    Usage {
        input_tokens: Some(11),
        output_tokens: Some(7),
        reasoning_tokens: Some(3),
        response_bytes: 100,
        wall_time_ms: 10,
        requested_model: model,
        reported_model: Some(MODEL.into()),
    }
}
fn snapshot(e: Uuid, c: Uuid) -> Value {
    admin().query_one("SELECT coalesce(jsonb_agg(jsonb_build_array(call,binding,state,usage) ORDER BY call),'[]') FROM execution.inference_calls WHERE e=$1 AND c=$2", &[&e,&c]).unwrap().get(0)
}
fn decision(
    slot: u128,
    profile: &str,
    exercise: impl FnOnce(&mut duskweave::m1_reasoning::postgres::PgBroker, &Scope, &Reservation, &[u8]),
) {
    let (e, c, _) = register(slot, profile);
    let run = Uuid::new_v4();
    let publication = live_publication(e, c, run);
    let mut runs = duskweave::postgres_m1_run::PgRunStore::new(broker());
    assert!(
        runs.start(
            &successor_request(&publication, run, "m1_autonomous_run_request_v3"),
            RELEASE
        )
        .unwrap()
    );
    let record = runs.inspect(run).unwrap();
    let renewals = Renewals::start(run);
    let prepared = prepare_decision_one(&mut runs, &record);
    let mut port = broker_port(
        &record,
        Instant::now() + Duration::from_millis(prepared.window_ms as u64),
    );
    let pack = prepared.pack.utf8().as_bytes();
    let r = port.reserve(&prepared.scope, pack).unwrap();
    exercise(&mut port, &prepared.scope, &r, pack);
    renewals.retire();
    runs.stop(run).unwrap();
}

#[test]
#[ignore = "dedicated schema-0033 worker only"]
fn sql_live_pro_intent_generation_and_repair_are_profile_bound() {
    assert!(worker());
    decision(0x73301, PRO, |port, s, r, pack| {
        for (call, codes) in [
            (r.generation_call, None),
            (r.repair_call, Some(&["proposal_shape"][..])),
        ] {
            let bound = inference_intent::bind(s, r, call, pack, codes).unwrap();
            let intent = inference_intent::verify(&bound).unwrap();
            assert_eq!(intent["model"], MODEL);
            for (field, value) in [
                ("model", json!("deepseek-flash")),
                ("protocol", json!("chat-completions")),
                ("recipient", json!("http://127.0.0.1/responses")),
            ] {
                let mut changed = intent.clone();
                changed[field] = value;
                rejected_intent(s, call, &bound, changed);
            }
            for (field, value) in [
                ("reasoning_effort", json!("high")),
                ("max_output_tokens", json!(4097)),
                ("stream", json!(true)),
            ] {
                let mut changed = intent.clone();
                changed["settings"][field] = value;
                rejected_intent(s, call, &bound, changed);
            }
            port.may_send(s, call, &bound).unwrap();
            port.completed(s, call, &usage(MODEL)).unwrap();
        }
    });
}
fn rejected_intent(
    s: &Scope,
    call: Uuid,
    original: &duskweave::m1_reasoning::request::BoundCall,
    mut intent: Value,
) {
    intent.sort_all_objects();
    let mut bound = duskweave::m1_reasoning::request::BoundCall {
        pack: original.pack.clone(),
        configuration: original.configuration.clone(),
        body: original.body.clone(),
        digest: original.digest.clone(),
    };
    bound.configuration = serde_json::to_vec(&intent).unwrap();
    bound.digest = inference_intent::digest(&bound.pack, &bound.configuration);
    assert!(inference_intent::verify(&bound).is_err());
    assert!(
        broker()
            .query_one(
                "SELECT execution.inference_may_send($1,$2,$3,$4,$5,$6,$7,$8)",
                &[
                    &s.engagement,
                    &s.campaign,
                    &s.decision,
                    &call,
                    &String::from_utf8_lossy(&bound.pack).into_owned(),
                    &String::from_utf8_lossy(&bound.configuration).into_owned(),
                    &"",
                    &bound.digest
                ]
            )
            .is_err()
    );
}

#[test]
#[ignore = "dedicated schema-0033 worker only"]
fn sql_live_pro_accounting_and_terminal_diagnostics_preserve_model_identity() {
    assert!(worker());
    for (index, profile, model) in [
        (0, PRO, MODEL),
        (1, "deepseek-genai-responses-low-v1", "deepseek-flash"),
    ] {
        decision(0x73310 + index, profile, |port, s, r, pack| {
            let bound = inference_intent::bind(s, r, r.generation_call, pack, None).unwrap();
            port.may_send(s, r.generation_call, &bound).unwrap();
            let before = snapshot(s.engagement, s.campaign);
            for wrong in [
                if model == MODEL {
                    "deepseek-flash"
                } else {
                    MODEL
                },
                "arbitrary-model",
            ] {
                assert!(port.completed(s, r.generation_call, &usage(wrong)).is_err());
                assert_eq!(snapshot(s.engagement, s.campaign), before);
            }
            port.completed(s, r.generation_call, &usage(model)).unwrap();
            let settled = snapshot(s.engagement, s.campaign);
            assert_eq!(settled[0][3]["requested_model"], model);
            assert!(port.completed(s, r.generation_call, &usage(model)).is_err());
            assert_eq!(snapshot(s.engagement, s.campaign), settled);
            let repair =
                inference_intent::bind(s, r, r.repair_call, pack, Some(&["proposal_shape"]))
                    .unwrap();
            port.may_send(s, r.repair_call, &repair).unwrap();
            struct Basis;
            impl proposal::ProposalBasis for Basis {
                fn reference_eligible(&self, _: &str) -> bool {
                    true
                }
                fn manifest_eligible(&self, _: &str) -> bool {
                    true
                }
                fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
                    Ok(())
                }
                fn safe_explanation(&self, _: &str) -> bool {
                    true
                }
            }
            let admission = proposal::admit(b"{}", &mut Basis);
            let diagnostic = admission.diagnostic().unwrap();
            let before_diagnostic = snapshot(s.engagement, s.campaign);
            for wrong in [
                if model == MODEL {
                    "deepseek-flash"
                } else {
                    MODEL
                },
                "arbitrary-model",
            ] {
                assert!(
                    port.completed_with_diagnostic(
                        s,
                        r.repair_call,
                        &usage(wrong),
                        Some(&diagnostic)
                    )
                    .is_err()
                );
                assert_eq!(snapshot(s.engagement, s.campaign), before_diagnostic);
            }
            port.completed_with_diagnostic(s, r.repair_call, &usage(model), Some(&diagnostic))
                .unwrap();
            assert!(
                snapshot(s.engagement, s.campaign)
                    .to_string()
                    .contains("proposal_admission_diagnostic_v1")
            );
        });
    }
    decision(0x73312, PRO, |port, s, r, pack| {
        let bound = inference_intent::bind(s, r, r.generation_call, pack, None).unwrap();
        port.may_send(s, r.generation_call, &bound).unwrap();
        let metadata = ResponseMetadata {
            status: ResponseStatus::Incomplete,
            incomplete_reason: Some(IncompleteReason::MaxOutputTokens),
            usage_availability: UsageAvailability::Reported,
        };
        let bad = ProviderReport {
            usage: usage("deepseek-flash"),
            provider_response: metadata.clone(),
        };
        assert!(
            port.uncertain_with_report(s, r.generation_call, "provider_incomplete", Some(&bad))
                .is_err()
        );
        let good = ProviderReport {
            usage: usage(MODEL),
            provider_response: metadata,
        };
        port.uncertain_with_report(s, r.generation_call, "provider_incomplete", Some(&good))
            .unwrap();
        assert_eq!(
            snapshot(s.engagement, s.campaign)[0][3]["requested_model"],
            MODEL
        );
    });
}

#[test]
#[ignore = "dedicated schema-0033 worker only"]
fn sql_live_pro_missing_key_refuses_and_key_free_recovery_preserves_identity() {
    assert!(worker());
    let (e, c, _) = register(0x73320, PRO);
    let run = Uuid::new_v4();
    let request = successor_request(
        &live_publication(e, c, run),
        run,
        "m1_autonomous_run_request_v3",
    );
    let input = std::env::temp_dir().join(format!("dw-pro-missing-key-{run}.json"));
    std::fs::write(&input, serde_json::to_vec(&request).unwrap()).unwrap();
    let output = command()
        .env_remove("DEEPSEEK_API_KEY")
        .env("DW_M1_RUN_CONTROL_ROOT", control_root())
        .args([
            "m1-run",
            "--input",
            input.to_str().unwrap(),
            "--timing-profile",
            "reasoning-bounded-v1",
        ])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    let receipt: Value =
        serde_json::from_str(text.lines().find(|l| l.starts_with('{')).unwrap()).unwrap();
    assert_eq!(
        receipt["recorded_decisions"][0]["disposition"],
        "live_key_missing"
    );
    let before = snapshot(e, c);
    for op in ["m1-run-inspect", "m1-run-recover"] {
        let (ok, recovered) = cli(op, run);
        assert!(ok);
        assert_eq!(recovered["run_id"], json!(run));
        assert_eq!(snapshot(e, c), before);
    }
    std::fs::remove_file(input).unwrap();
}
