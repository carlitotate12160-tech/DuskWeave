//! Schema-0035 qualified LIVE binding tests at start and current use
//! (private_live_deadline) using synthetic v6 production Pro-v2 consent.
use super::super::fixture::*;
use super::super::receipt::LabBasis;
use super::super::run_support::*;
use super::{PRO_ENDPOINT, set_pro_v2_limits};
use duskweave::Fail;
use duskweave::m1_effect_session::EffectStore;
use duskweave::m1_inference_profile as profiles;
use duskweave::m1_reasoning::{
    inference::{self, Provider, Received},
    inference_intent,
    postgres::{Basis, PgDecisionStore},
    postgres_terrain::TerrainBasis,
    proposal::ProposalBasis,
    request::{self, BoundCall},
    response,
};
use duskweave::m1_run::RunPhase;
use duskweave::postgres_m1_effect::PgEffectStore;
use duskweave::postgres_m1_run::PgRunStore;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Default)]
pub(super) struct LiveProV2SyntheticProvider;

impl Provider for LiveProV2SyntheticProvider {
    fn prepare(&mut self, call: &BoundCall, _: Instant) -> Result<(), &'static str> {
        inference_intent::verify(call).map(|_| ())
    }

    fn call(
        &mut self,
        call: &BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        release()?;
        let pack: Value = serde_json::from_slice(&call.pack).map_err(|_| "pack_decode")?;
        let mut p = genai::proposal(&pack);
        p["next_intent"] = json!({
            "category": "observe_more",
            "references": ["mission", "asset_0"],
            "manifest_reference": "dns_udp",
            "view_request": null
        });
        let raw = serde_json::to_vec(&genai::envelope(&p)).map_err(|_| "envelope_encode")?;
        let mut received = response::admit_envelope(&raw, raw.len(), Duration::ZERO, basis)?;
        received.usage.requested_model = super::PRO_MODEL;
        Ok(received)
    }
}

pub(super) fn register_live_pro_v2(
    slot: u128,
    run_profile: &str,
    inf_profile: &str,
    endpoint: &str,
    resolver: &str,
) -> (Uuid, Uuid, Uuid) {
    register_shaped(slot, |raw| {
        let p = &mut raw["m1_inference_permission"];
        p["version"] = json!("m1_inference_permission_v6");
        p["execution_mode"] = json!("live");
        p["run_profile"] = json!(run_profile);
        p["profile"] = json!(inf_profile);
        p["recipient"] = json!(endpoint);
        p["live_binding"] = json!({
            "qualification_ref": Uuid::new_v4(),
            "release_sha256": RELEASE,
            "resolver_ipv4": resolver
        });
        set_pro_v2_limits(p);
        raw["m1_permission"]["resolver_ipv4"] = json!(resolver);
    })
}

pub(super) fn effect_and_run_counts(e: Uuid) -> (i64, i64, i64) {
    let mut a = admin();
    let runs: i64 = a
        .query_one(
            "SELECT count(*) FROM execution.autonomous_runs WHERE e=$1",
            &[&e],
        )
        .unwrap()
        .get(0);
    let calls: i64 = a
        .query_one(
            "SELECT count(*) FROM execution.inference_calls WHERE e=$1",
            &[&e],
        )
        .unwrap()
        .get(0);
    let effects: i64 = a
        .query_one(
            "SELECT count(*) FROM execution.live_effect_sessions WHERE e=$1",
            &[&e],
        )
        .unwrap()
        .get(0);
    (runs, calls, effects)
}

fn setup_reserved_live_pro_v2_effect(
    slot: u128,
    resolver: &str,
) -> (
    Uuid,
    Uuid,
    Uuid,
    duskweave::m1_effect_session::EffectSession,
) {
    let (e, c, _) = register_live_pro_v2(
        slot,
        profiles::LIVE_RUN_PROFILE,
        profiles::PRO_V2_PROFILE,
        PRO_ENDPOINT,
        resolver,
    );
    let run = Uuid::new_v4();
    let publication = live_publication(e, c, run);
    let qual_ref = seed_live_qualification(&publication);

    let mut runs = PgRunStore::new(broker());
    let req = successor_request(&publication, run, "m1_autonomous_run_request_v4");
    assert!(matches!(runs.start(&req, RELEASE), Ok(true)));
    runs.check(run)
        .expect("run_current succeeds under live Pro-v2");
    let record = runs.inspect(run).unwrap();
    let prepared = prepare_decision_one(&mut runs, &record);
    let deadline = Instant::now() + Duration::from_millis(prepared.window_ms as u64);
    let pack = prepared.pack.utf8().as_bytes();

    let mut port = broker_port(&record, deadline);
    let mut provider = LiveProV2SyntheticProvider;
    let mut basis = LabBasis(TerrainBasis {
        mission: Basis {
            client: broker(),
            scope: prepared.scope.clone(),
            references: request::reference_ids(pack).unwrap(),
            deadline,
        },
        dependencies: prepared.pack.terrain_dependencies().to_vec(),
    });
    let receipt = inference::infer(
        &mut port,
        &mut provider,
        &mut basis,
        &mut PgDecisionStore {
            client: client("DW_COGNITION_DATABASE_URL"),
            input_hash: prepared.input_hash.clone(),
        },
        &mut duskweave::postgres_trajectory::PgTrajectory::new(client(
            "DW_TRAJECTORY_DATABASE_URL",
        )),
        inference::InferenceInput {
            scope: &prepared.scope,
            pack,
            event_id: Uuid::new_v4(),
            occurred_at_ms: now_ms() as u64,
            source_mode: "blind".into(),
        },
    );
    assert_eq!(receipt.proposal_disposition, "validated_proposal");
    assert_eq!(receipt.publication, "committed");
    assert_eq!(receipt.history, "completed");
    runs.complete_phase(run, RunPhase::DecisionOne).unwrap();
    runs.start_phase(run, RunPhase::Acquisition).unwrap();
    runs.claim_phase(run, RunPhase::Acquisition, RELEASE)
        .unwrap();
    runs.check(run).unwrap();

    let mut effect = PgEffectStore::new(broker());
    let session = effect.reserve(run).unwrap();
    (e, qual_ref, run, session)
}

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_pro_v2_live_qualification_start_and_current() {
    assert!(worker());
    assert_eq!(std::env::var_os("DEEPSEEK_API_KEY"), None);

    // 1. Valid qualified LIVE Pro-v2 reaches private_live_deadline via current_deadline & may_start
    //    without launching any external DNS adapter.
    let (e_ok, qual_ok, run_ok, session_ok) = setup_reserved_live_pro_v2_effect(0xac01, "9.9.9.9");
    let mut effect = PgEffectStore::new(broker());
    let dl1 = effect
        .current_deadline(&session_ok)
        .expect("valid current_deadline");
    assert!(dl1 > 0);
    let dl2 = effect
        .may_start(&session_ok)
        .expect("valid may_start reaches private_live_deadline");
    assert!(dl2 > 0);
    assert_eq!(
        effect.may_start(&session_ok),
        Err(Fail::State("live_effect_release_refused")),
        "second may_start must refuse"
    );
    PgRunStore::new(broker()).stop(run_ok).unwrap();
    let _ = (e_ok, qual_ok);

    // 2. Post-start qualification expiry: run_current stays separate, while private_live_deadline
    //    refuses at current_deadline and may_start with zero may_start event or live_dns_results.
    let (_e_exp, qual_exp, run_exp, session_exp) =
        setup_reserved_live_pro_v2_effect(0xac02, "9.9.9.9");
    admin()
        .execute(
            "UPDATE execution.live_qualifications SET valid_from_ms = 1, valid_until_ms = 2 WHERE qualification_ref = $1",
            &[&qual_exp],
        )
        .unwrap();
    PgRunStore::new(broker())
        .check(run_exp)
        .expect("run_current remains independent of host qualification expiry");
    assert_eq!(
        effect.current_deadline(&session_exp),
        Err(Fail::State("live_effect_release_refused"))
    );
    assert_eq!(
        effect.may_start(&session_exp),
        Err(Fail::State("live_effect_release_refused"))
    );
    let may_start_events: i64 = admin()
        .query_one(
            "SELECT count(*) FROM execution.live_effect_events WHERE run_id=$1 AND phase='may_start'",
            &[&run_exp],
        )
        .unwrap()
        .get(0);
    assert_eq!(may_start_events, 0);
    PgRunStore::new(broker()).stop(run_exp).unwrap();

    // 3. Post-start qualification repudiation (release_sha256 mismatch): private_live_deadline refuses.
    let (_e_rep, qual_rep, run_rep, session_rep) =
        setup_reserved_live_pro_v2_effect(0xac03, "9.9.9.9");
    admin()
        .execute(
            "UPDATE execution.live_qualifications SET release_sha256 = repeat('c', 64) WHERE qualification_ref = $1",
            &[&qual_rep],
        )
        .unwrap();
    assert_eq!(
        effect.current_deadline(&session_rep),
        Err(Fail::State("live_effect_release_refused"))
    );
    assert_eq!(
        effect.may_start(&session_rep),
        Err(Fail::State("live_effect_release_refused"))
    );
    PgRunStore::new(broker()).stop(run_rep).unwrap();

    // 4. Start-time qualification refusals: missing, expired, wrong release, wrong purpose, wrong resolver.
    for (slot, mutate, label) in [
        (0xac10_u128, "missing", "missing_qualification"),
        (0xac11, "expired", "expired_qualification"),
        (0xac12, "release", "wrong_release"),
        (0xac13, "purpose", "wrong_purpose_lab_on_live"),
        (0xac14, "resolver", "wrong_resolver"),
    ] {
        let (e, c, _) = register_live_pro_v2(
            slot,
            profiles::LIVE_RUN_PROFILE,
            profiles::PRO_V2_PROFILE,
            PRO_ENDPOINT,
            "9.9.9.9",
        );
        let run = Uuid::new_v4();
        let pub_val = live_publication(e, c, run);
        if mutate != "missing" {
            let qref = seed_live_qualification(&pub_val);
            match mutate {
                "expired" => {
                    admin()
                        .execute(
                            "UPDATE execution.live_qualifications SET valid_from_ms = 1, valid_until_ms = 2 WHERE qualification_ref = $1",
                            &[&qref],
                        )
                        .unwrap();
                }
                "release" => {
                    admin()
                        .execute(
                            "UPDATE execution.live_qualifications SET release_sha256 = repeat('d', 64) WHERE qualification_ref = $1",
                            &[&qref],
                        )
                        .unwrap();
                }
                "purpose" => {
                    admin()
                        .execute(
                            "DELETE FROM execution.live_qualifications WHERE qualification_ref = $1",
                            &[&qref],
                        )
                        .unwrap();
                    let lab_q = json!({
                        "fields": {
                            "m1_inference_permission": {
                                "run_profile": LAB,
                                "live_binding": {
                                    "qualification_ref": qref,
                                    "release_sha256": RELEASE,
                                    "resolver_ipv4": "192.0.2.53"
                                }
                            }
                        }
                    });
                    seed_live_qualification(&lab_q);
                }
                "resolver" => {
                    admin()
                        .execute(
                            "UPDATE execution.live_qualifications SET resolver_ipv4 = '8.8.8.8'::inet WHERE qualification_ref = $1",
                            &[&qref],
                        )
                        .unwrap();
                }
                _ => unreachable!(),
            }
        }
        let req = successor_request(&pub_val, run, "m1_autonomous_run_request_v4");
        let err = PgRunStore::new(broker()).start(&req, RELEASE).unwrap_err();
        assert_eq!(err, Fail::State("run_start_refused"), "{label}");
        assert_eq!(
            effect_and_run_counts(e),
            (0, 0, 0),
            "{label}: zero mutations"
        );
    }

    // 5. LAB + Pro-v2 and Mirror + LIVE must be refused at both Rust and SQL boundaries
    //    starting from valid registered publication-derived campaigns with current qualification.
    let (lab_ctrl_e, lab_ctrl_c, _) = register_live(0xac1f, true);
    let lab_ctrl_run = Uuid::new_v4();
    let lab_ctrl_pub = live_publication(lab_ctrl_e, lab_ctrl_c, lab_ctrl_run);
    seed_live_qualification(&lab_ctrl_pub);
    let lab_ctrl_req =
        successor_request(&lab_ctrl_pub, lab_ctrl_run, "m1_autonomous_run_request_v4");
    assert!(matches!(
        PgRunStore::new(broker()).start(&lab_ctrl_req, RELEASE),
        Ok(true)
    ));
    PgRunStore::new(broker()).stop(lab_ctrl_run).unwrap();

    for (slot, run_profile, base_inf_profile, bad_inf_profile, resolver) in [
        (
            0xac20_u128,
            LAB,
            "fixture-genai-responses-v1",
            profiles::PRO_V2_PROFILE,
            "192.0.2.53",
        ),
        (
            0xac21,
            profiles::LIVE_RUN_PROFILE,
            profiles::PRO_V2_PROFILE,
            profiles::FIXTURE_PRO_V2_PROFILE,
            "9.9.9.9",
        ),
        (
            0xac22,
            LAB,
            "fixture-genai-responses-v1",
            profiles::FIXTURE_PRO_V2_PROFILE,
            "192.0.2.53",
        ),
    ] {
        assert!(!profiles::live_binding(
            run_profile,
            "live",
            bad_inf_profile
        ));
        let (e, c, _) = if run_profile == LAB {
            register_live(slot, true)
        } else {
            register_live_pro_v2(slot, run_profile, base_inf_profile, PRO_ENDPOINT, resolver)
        };
        let run = Uuid::new_v4();
        let pub_val = live_publication(e, c, run);
        seed_live_qualification(&pub_val);

        let mut bad_perm = pub_val["fields"]["m1_inference_permission"].clone();
        bad_perm["profile"] = json!(bad_inf_profile);
        bad_perm["recipient"] = json!(if bad_inf_profile == profiles::FIXTURE_PRO_V2_PROFILE {
            super::MIRROR_ENDPOINT
        } else {
            PRO_ENDPOINT
        });
        set_pro_v2_limits(&mut bad_perm);
        assert!(!super::sql_matches(&bad_perm));

        let mut raw_req = serde_json::to_value(successor_request(
            &pub_val,
            run,
            "m1_autonomous_run_request_v4",
        ))
        .unwrap();
        raw_req["inference_profile"] = json!(bad_inf_profile);
        assert!(
            duskweave::m1_run::RunRequest::parse(serde_json::to_vec(&raw_req).unwrap().as_slice())
                .is_err()
        );
        let sql_err = broker()
            .query_one("SELECT execution.run_start($1,$2)", &[&raw_req, &RELEASE])
            .unwrap_err();
        assert!(
            db_message(&sql_err).contains("run_request_refused")
                || db_message(&sql_err).contains("run_authority_unavailable"),
            "unexpected SQL error for {run_profile} + {bad_inf_profile}: {sql_err}"
        );
        assert_eq!(
            effect_and_run_counts(e),
            (0, 0, 0),
            "{run_profile} + {bad_inf_profile}: zero mutations"
        );
    }
}
