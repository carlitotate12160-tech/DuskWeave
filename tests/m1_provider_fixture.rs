//! Offline owner composition. The scripted Provider is not real cognition or a production fallback.
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/database_wait_proxy.rs"]
mod proxy;
#[path = "support/m1_terrain_context.rs"]
mod terrain;
use terrain::terrain_support::native;
#[path = "support/m1_run.rs"]
mod run_support;
#[path = "m1_provider_fixture/supervision.rs"]
mod supervision;
#[path = "m1_provider_fixture/withdrawal.rs"]
mod withdrawal;
use duskweave::m1_reasoning::{
    inference::{self, Provider, Scope},
    postgres::{Basis, PgBroker, PgDecisionStore},
    postgres_terrain::TerrainBasis,
    proposal::ProposalBasis,
    request,
};
use duskweave::m1_run::{RunPhase, RunRecord, RunRequest};
use duskweave::mission::OperationId;
use duskweave::postgres_inference_budget::PgInferenceReservationStore;
use duskweave::postgres_m1_terrain::PgTerrainStore;
use duskweave::postgres_mission::{PgContextReader, PgMissionStore};
use duskweave::postgres_terrain_history::record_terrain_history;
use duskweave::postgres_trajectory::PgTrajectory;
use run_support::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use uuid::Uuid;

fn provider_permission(raw: &mut Value) {
    let p = &mut raw["m1_inference_permission"];
    p["run_profile"] = json!("autonomous-dns-provider-fixture-v1");
    p["profile"] = json!("deepseek-responses-autonomous-v1");
    p["recipient"] = json!("https://api.deepseek.com/responses");
}
fn eight_seeds(raw: &mut Value) {
    provider_permission(raw);
    let rule = raw["m1_permission"]["contact_rules"][0].clone();
    let assets: Vec<_> = (0..8)
        .map(|i| Uuid::from_u128(if i == 0 { 0x21 } else { 0x100 + i }))
        .collect();
    raw["included_assets"] = json!(assets);
    raw["m1_permission"]["contact_rules"] = json!(
        assets
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let mut r = rule.clone();
                r["asset_ref"] = json!(id);
                r["priority"] = json!(i + 1);
                if i > 0 {
                    r["rule"] = json!({"exact":format!("seed{i}.example.invalid")});
                }
                r
            })
            .collect::<Vec<_>>()
    );
}
#[test]
fn provider_tuple_registers_and_both_rust_and_restricted_sql_reject_rebinding() {
    let _guard = native::db();
    refresh();
    let q = registered_with(0xc901, provider_permission);
    let parsed = RunRequest::parse(&serde_json::to_vec(&q).unwrap()).unwrap();
    let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    for (profile, mode) in [
        ("autonomous-dns-fixture-v1", "controlled_fixture"),
        ("autonomous-dns-provider-fixture-v1", "live"),
        ("autonomous-dns-v1", "live"),
        ("arbitrary", "controlled_fixture"),
    ] {
        let mut bad = q.clone();
        bad["run_profile"] = json!(profile);
        bad["execution_mode"] = json!(mode);
        let mut tx = broker.transaction().unwrap();
        assert!(
            tx.query_one("SELECT execution.run_start($1,$2)", &[&bad, &release()])
                .is_err()
        );
        tx.rollback().unwrap();
    }
    let rec = start_only(&q);
    assert_eq!(rec.request.run_profile, parsed.run_profile);
    assert_eq!(
        broker_store().summary(id(&q)).unwrap()["run_profile"],
        q["run_profile"]
    );
    broker_store().stop(id(&q)).unwrap();
    assert!(broker_store().check(id(&q)).is_err());
}

fn raw_registration(template: &Value, slot: u128, mutate: impl FnOnce(&mut Value)) -> Value {
    let old_e = Uuid::parse_str(
        template["context"]["mission"]["engagement_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let old_c = Uuid::parse_str(
        template["context"]["mission"]["campaign_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let mut core = native::client("DW_TEST_DATABASE_URL");
    let mut source: Value = core.query_one(
        "SELECT contract FROM mission.registration_outbox WHERE engagement_id=$1 AND campaign_id=$2",
        &[&old_e,&old_c]).unwrap().get(0);
    let (e, c) = db_support::scope(slot);
    let operation = consent_id();
    let event = consent_id();
    for (key, value) in [
        ("engagement_id", json!(e)),
        ("campaign_id", json!(c)),
        ("operation_id", json!(operation)),
        ("causation_id", json!(operation)),
        ("correlation_id", json!(operation)),
        ("event_id", json!(event)),
        ("affected_entity", json!(c)),
    ] {
        source[key] = value;
    }
    source["fields"]["m1_inference_permission"]["engagement_id"] = json!(e);
    source["fields"]["m1_inference_permission"]["campaign_id"] = json!(c);
    mutate(&mut source["fields"]["m1_inference_permission"]);
    core.execute("INSERT INTO mission.missions \
        (engagement_id,campaign_id,operator_ref,authority_ref,authority_revision,goal_ref,included_assets,excluded_assets,exercise_mode,starts_at,ends_at,revision,operation_id) \
        SELECT $1,$2,operator_ref,authority_ref,authority_revision,goal_ref,included_assets,excluded_assets,exercise_mode,starts_at,ends_at,revision,$3 \
        FROM mission.missions WHERE engagement_id=$4 AND campaign_id=$5",
        &[&e.0,&c.0,&operation,&old_e,&old_c]).unwrap();
    core.execute("INSERT INTO mission.registration_outbox(engagement_id,campaign_id,operation_id,event_id,contract) VALUES($1,$2,$3,$4,$5)",
        &[&e.0,&c.0,&operation,&event,&source]).unwrap();
    core.execute("INSERT INTO trajectory.registration_history(engagement_id,campaign_id,producer,operation_id,event_id,status,contract) \
        VALUES($1,$2,'mission',$3,$4,'accepted',$5)",&[&e.0,&c.0,&operation,&event,&source]).unwrap();
    let mut q = template.clone();
    q["run_id"] = json!(consent_id());
    q["context"]["mission"]["engagement_id"] = json!(e);
    q["context"]["mission"]["campaign_id"] = json!(c);
    q
}

#[test]
fn raw_sql_run_admission_requires_explicit_current_terms_and_complete_disclosure() {
    let _guard = native::db();
    refresh();
    let template = registered_with(0xc910, provider_permission);
    let cases: Vec<(&str, Value)> = vec![
        ("provider_terms_approved", json!(false)),
        ("provider_terms_ref", Value::Null),
        ("provider_terms_ref", json!(Uuid::nil())),
        ("recipient", json!("urn:duskweave:fixture:inference:v1")),
        ("profile", json!("deterministic-autonomous-v1")),
        ("version", json!("m1_inference_permission_v2")),
        ("ends_at", json!(1)),
        (
            "field_classes",
            json!([
                "mission_question_constraints",
                "scoped_opaque_references",
                "owner_qualified_summaries",
                "admitted_operator_premises",
                "manifest_availability",
                "omissions_unknown",
                "capacity",
                "admitted_repair_fields_codes"
            ]),
        ),
        (
            "campaign_limits",
            json!({"calls":3,"request_bytes":65536,"response_bytes":262144,
            "output_token_capacity":16384,"wall_time_ms":90000}),
        ),
    ];
    for (i, (field, value)) in cases.into_iter().enumerate() {
        let q = raw_registration(&template, 0xc920 + i as u128, |p| p[field] = value);
        let mut c = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
        let mut tx = c.transaction().unwrap();
        let accepted = tx
            .query_one("SELECT execution.run_start($1,$2)", &[&q, &release()])
            .is_ok();
        tx.rollback().unwrap();
        assert!(
            !accepted,
            "raw SQL admitted invalid {field} before provider preparation"
        );
    }
}

struct Scripted;
impl Provider for Scripted {
    fn prepare(&mut self, b: &request::BoundCall, deadline: Instant) -> Result<(), &'static str> {
        let cfg: Value = serde_json::from_slice(&b.configuration).unwrap();
        assert_eq!(cfg["profile"], "deepseek-responses-autonomous-v1");
        assert_eq!(cfg["requested_settings"]["model"], "deepseek-flash");
        assert!(deadline > Instant::now());
        reject_rebound_calls(b);
        Ok(())
    }
    fn call(
        &mut self,
        b: &request::BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<inference::Received, &'static str> {
        release()?;
        let pack: Value = serde_json::from_slice(&b.pack).unwrap();
        let second = pack["predecessor"]["availability"] == "committed";
        let seed = pack["candidates"][0]["reference"].clone();
        let mut refs = vec![seed.clone()];
        if second {
            refs.push(json!("previous_decision"));
            refs.push(
                pack["references"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["source"] == "fixture_dns_acquisition")
                    .unwrap()["id"]
                    .clone(),
            );
        }
        let hypothesis = if second {
            "Fixture feedback retains bounded uncertainty.".into()
        } else {
            "\u{00e9}".repeat(64)
        };
        let proposal = json!({"version":"m1_inference_proposal_v1","hypothesis":hypothesis,
            "considered_alternative":"Routing may be unavailable.","references":refs,
            "expected_observation":"Fixture routing metadata.","falsifying_observation":"Expired owner evidence.",
            "uncertainty":"Not a live target fact.","rationale":"Use only the current admitted references.",
            "next_intent":{"category":if second {"retain"} else {"observe_more"},
            "references":[seed],"manifest_reference":if second {Value::Null} else {json!("dns_udp")}}});
        let raw = serde_json::to_vec(&json!({"status":"completed","model":"deepseek-flash",
            "output":[{"type":"message","role":"assistant","status":"completed",
            "content":[{"type":"output_text","text":proposal.to_string()}]}]}))
        .unwrap();
        duskweave::m1_reasoning::response::admit_envelope(&raw, raw.len(), Duration::ZERO, basis)
    }
}
struct OwnerBasis(TerrainBasis);
impl ProposalBasis for OwnerBasis {
    fn reference_eligible(&self, r: &str) -> bool {
        self.0.reference_eligible(r)
    }
    fn manifest_eligible(&self, r: &str) -> bool {
        r == "dns_udp"
    }
    fn safe_explanation(&self, s: &str) -> bool {
        self.0.safe_explanation(s)
    }
    fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
        self.0.current_authority_and_premises()
    }
    fn before_provider_release(&mut self) -> Result<(), &'static str> {
        self.0.before_provider_release()
    }
    fn current_authority_for_references(&mut self, r: &[String]) -> Result<(), &'static str> {
        self.0.current_authority_for_references(r)
    }
}
fn infer(rec: &RunRecord, phase: RunPhase) -> Vec<u8> {
    let mut run = broker_store();
    let r = rec.request.run_id;
    run.renew(r).unwrap();
    run.start_phase(r, phase).unwrap();
    run.claim_phase(r, phase, &release()).unwrap();
    let q = &rec.request.context;
    let d = if phase == RunPhase::DecisionOne {
        rec.decision_one
    } else {
        rec.decision_two
    };
    let hash = Sha256::digest(serde_json::to_vec(q).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let s = Scope {
        engagement: q.mission.engagement_id.0,
        campaign: q.mission.campaign_id.0,
        decision: d,
        mission_revision: 1,
        permission: q.mission.permission_id,
    };
    native::client("DW_INFERENCE_BROKER_DATABASE_URL")
        .execute(
            "SELECT execution.bind_inference_input($1,$2,$3,$4)",
            &[&s.engagement, &s.campaign, &d, &hash],
        )
        .unwrap();
    let a = duskweave::m1_autonomous_context::AutonomousContext {
        binding: duskweave::m1_autonomous_context::RunContextBinding {
            engagement_id: q.mission.engagement_id,
            campaign_id: q.mission.campaign_id,
            run_profile: rec.request.run_profile.clone(),
            execution_mode: rec.request.execution_mode.clone(),
            release_sha256: rec.release_sha256.clone(),
            qualification_ref: rec.qualification_ref,
        },
        predecessor: if phase == RunPhase::DecisionTwo {
            Some(run.predecessor(r).unwrap())
        } else {
            None
        },
    };
    let p = duskweave::m1_context::prepare_autonomous(
        &mut PgContextReader::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL")),
        q,
        OperationId(d),
        &a,
    )
    .unwrap();
    assert_producer_wire(rec, &s, p.utf8());
    infer_prepared(q, &s, &p, hash);
    run.complete_phase(r, phase).unwrap();
    p.utf8().as_bytes().to_vec()
}
fn infer_prepared(
    q: &duskweave::m1_context_input::ContextRequest,
    s: &Scope,
    p: &duskweave::m1_context::PreparedContext,
    hash: String,
) {
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut broker = PgBroker {
        client: native::client("DW_INFERENCE_BROKER_DATABASE_URL"),
        mission: PgMissionStore::new(native::client("DW_TEST_DATABASE_URL")),
        reservations: PgInferenceReservationStore::new(native::client(
            "DW_INFERENCE_BROKER_DATABASE_URL",
        )),
        request: q.mission.clone(),
        deadline,
    };
    let mut basis = OwnerBasis(TerrainBasis {
        mission: Basis {
            client: native::client("DW_INFERENCE_BROKER_DATABASE_URL"),
            scope: s.clone(),
            references: request::reference_ids(p.utf8().as_bytes()).unwrap(),
            deadline,
        },
        dependencies: p.terrain_dependencies().to_vec(),
    });
    let mut store = PgDecisionStore {
        client: native::client("DW_COGNITION_DATABASE_URL"),
        input_hash: hash,
    };
    let input = inference::InferenceInput {
        scope: s,
        pack: p.utf8().as_bytes(),
        event_id: consent_id(),
        occurred_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        source_mode: "blind".into(),
    };
    let receipt = inference::infer(
        &mut broker,
        &mut Scripted,
        &mut basis,
        &mut store,
        &mut PgTrajectory::new(native::client("DW_TRAJECTORY_DATABASE_URL")),
        input,
    );
    assert_eq!(receipt.proposal_disposition, "validated_proposal");
    assert_eq!(receipt.history, "completed");
}
fn reject_rebound_calls(b: &request::BoundCall) {
    let p: Value = serde_json::from_slice(&b.pack).unwrap();
    let e = Uuid::parse_str(p["engagement_id"].as_str().unwrap()).unwrap();
    let c = Uuid::parse_str(p["campaign_id"].as_str().unwrap()).unwrap();
    let d = Uuid::parse_str(p["decision_id"].as_str().unwrap()).unwrap();
    let mut client = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let k: Uuid = client
        .query_one(
            "SELECT generation_call_id FROM execution.inference_reservations \
        WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3 AND record_revision=1",
            &[&e, &c, &d],
        )
        .unwrap()
        .get(0);
    let cfg: Value = serde_json::from_slice(&b.configuration).unwrap();
    for field in ["run_profile", "execution_mode", "profile", "recipient"] {
        let mut wrong = cfg.clone();
        wrong[field] = json!("substituted");
        let wrong = serde_json::to_vec(&wrong).unwrap();
        let digest = request::compute_binding(&b.pack, &wrong, &b.body);
        let mut tx = client.transaction().unwrap();
        assert!(
            tx.query_one(
                "SELECT execution.inference_may_send($1,$2,$3,$4,$5,$6,$7,$8)",
                &[
                    &e,
                    &c,
                    &d,
                    &k,
                    &std::str::from_utf8(&b.pack).unwrap(),
                    &std::str::from_utf8(&wrong).unwrap(),
                    &std::str::from_utf8(&b.body).unwrap(),
                    &digest
                ]
            )
            .is_err(),
            "rebound {field}"
        );
        tx.rollback().unwrap();
    }
}
fn assert_producer_wire(rec: &RunRecord, s: &Scope, pack: &str) {
    for repair in [None, Some(&["unexpected_view_request"; 8][..])] {
        let call = request::bind(pack.as_bytes(), repair).unwrap();
        request::verify_binding(
            pack,
            std::str::from_utf8(&call.configuration).unwrap(),
            std::str::from_utf8(&call.body).unwrap(),
            &call.digest,
            repair.is_some(),
        )
        .unwrap();
        assert!(
            call.pack.len() <= 8192
                && call.body.len() <= 16384
                && call.configuration.len() <= 16384
        );
        println!(
            "offline real producer run={} decision={} P={} C={} B={} repair={}",
            rec.request.run_id,
            s.decision,
            call.pack.len(),
            call.configuration.len(),
            call.body.len(),
            repair.is_some()
        );
    }
}
fn publish_fixture(rec: &RunRecord) {
    let mut broker = broker_store();
    let r = rec.request.run_id;
    broker.renew(r).unwrap();
    broker.start_phase(r, RunPhase::Acquisition).unwrap();
    broker
        .claim_phase(r, RunPhase::Acquisition, &release())
        .unwrap();
    let result = duskweave::m1_fixture_dns::acquire(broker.reserve_effect(r, 0).unwrap()).unwrap();
    broker.commit_result(r, &result).unwrap();
    let m = &rec.request.context.mission;
    let mut tr = native::client("DW_TERRAIN_DATABASE_URL");
    let reference = json!({"version":"m1_terrain_dns_issued_observation_v1","engagement_id":m.engagement_id,
        "campaign_id":m.campaign_id,"result_ref":rec.result,"run_profile":rec.request.run_profile,
        "execution_mode":rec.request.execution_mode});
    for (key, value) in [
        ("run_profile", json!("autonomous-dns-fixture-v1")),
        ("execution_mode", json!("live")),
        ("result_ref", json!(rec.attempt)),
        ("campaign_id", json!(consent_id())),
    ] {
        let mut bad = reference.clone();
        bad[key] = value;
        assert!(
            tr.query_one("SELECT terrain.issued_dns_request($1)", &[&bad])
                .is_err()
        );
    }
    let typed = serde_json::from_value(reference).unwrap();
    let event = PgTerrainStore::new(tr)
        .record_issued(
            &mut PgMissionStore::new(native::client("DW_TEST_DATABASE_URL")),
            &typed,
        )
        .unwrap()
        .unwrap()
        .0;
    assert_eq!(
        record_terrain_history(&mut native::client("DW_TRAJECTORY_DATABASE_URL"), &event).unwrap(),
        duskweave::trajectory::HistoryStatus::Completed
    );
    broker.complete_phase(r, RunPhase::Acquisition).unwrap();
}
#[test]
fn real_producers_and_owners_bind_two_packs_to_original_fixture_and_recovery_never_replays() {
    let _guard = native::db();
    refresh();
    for (slot, seeds) in [(0xc902, 1), (0xc904, 8)] {
        causal_feedback(slot, seeds);
    }
}
fn causal_feedback(slot: u128, seeds: usize) {
    let q = registered_with(slot, |raw| {
        if seeds == 8 {
            eight_seeds(raw)
        } else {
            provider_permission(raw)
        }
    });
    let rec = start_only(&q);
    let custodian = LeaseCustodian::start(&rec, broker_store());
    let first = infer(&rec, RunPhase::DecisionOne);
    publish_fixture(&rec);
    let second = infer(&rec, RunPhase::DecisionTwo);
    assert_ne!(first, second);
    let p: Value = serde_json::from_slice(&second).unwrap();
    assert_eq!(p["predecessor"]["result_ref"], json!(rec.result));
    assert_eq!(p["predecessor"]["attempt_ref"], json!(rec.attempt));
    let reference = p["references"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == "fixture_dns_acquisition")
        .unwrap();
    let issued: Value = native::admin()
        .query_one(
            "SELECT request FROM execution.fixture_dns_results WHERE result=$1",
            &[&rec.result],
        )
        .unwrap()
        .get(0);
    assert_eq!(
        reference["perceived_at"], issued["perceived_at"],
        "owner intake cannot refresh perception time"
    );
    assert_eq!(reference["ttl_seconds"], 60);
    assert_eq!(
        reference["valid_until"],
        json!(issued["perceived_at"].as_i64().unwrap() + 60)
    );
    let before = broker_store().summary(rec.request.run_id).unwrap();
    assert_eq!(before["effect_count"], 1);
    assert_eq!(before["decision_count"], 2);
    assert_eq!(before["inference"].as_array().unwrap().len(), 2);
    assert!(
        !broker_store()
            .start(&serde_json::from_value(q).unwrap(), &release())
            .unwrap()
    );
    let (ok, after) = cli("m1-run-recover", "--run", &rec.request.run_id.to_string());
    assert!(ok, "{after}");
    assert_eq!(before["inference"], after["inference"]);
    assert_eq!(before["effect_count"], after["effect_count"]);
    let _ = custodian.finish();
}

#[test]
fn minimum_wire_fit_failure_precedes_reservation_provider_preparation_and_fixture_effect() {
    let _guard = native::db();
    refresh();
    let q = registered_with(0xc943, |raw| {
        provider_permission(raw);
        raw["m1_inference_permission"]["operation_limits"]["context_bytes"] = json!(1);
    });
    let root = control_root();
    let path = path(&q);
    let output = command()
        .env("DW_M1_RUN_CONTROL_ROOT", &root)
        .args(["m1-run", "--input", path.to_str().unwrap()])
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!output.status.success());
    let rec = broker_store().inspect(id(&q)).unwrap();
    let summary = broker_store().summary(id(&q)).unwrap();
    assert_eq!(summary["inference"], json!([]));
    assert_eq!(summary["effect_count"], 0);
    assert_eq!(summary["decision_count"], 0);
    let count: i64 = native::admin()
        .query_one(
            "SELECT count(*) FROM execution.inference_reservations WHERE operation_id IN($1,$2)",
            &[&rec.decision_one, &rec.decision_two],
        )
        .unwrap()
        .get(0);
    assert_eq!(
        count, 0,
        "minimum useful context must fit before capacity is charged"
    );
}

// Preserve test entrypoint names used by CI exact filters and recorded commands.

#[test]
fn normal_binary_missing_control_refuses_provider_phase_without_deterministic_fallback() {
    supervision::normal_binary_missing_control_refuses_provider_phase_without_deterministic_fallback();
}

#[test]
fn qualified_private_control_reaches_selected_provider_and_missing_key_is_durable_not_fallback() {
    supervision::qualified_private_control_reaches_selected_provider_and_missing_key_is_durable_not_fallback();
}

#[test]
fn production_stop_withdraw_and_manual_terrain_signal_before_blocked_authority_sql() {
    supervision::production_stop_withdraw_and_manual_terrain_signal_before_blocked_authority_sql();
}

#[test]
fn stalled_claim_reply_exits_provider_child_from_the_launch_clock() {
    supervision::stalled_claim_reply_exits_provider_child_from_the_launch_clock();
}

#[test]
fn stalled_reservation_reply_hard_stops_at_the_original_decision_envelope() {
    supervision::stalled_reservation_reply_hard_stops_at_the_original_decision_envelope();
}

#[test]
fn fenced_epoch_bump_serializes_first_attempt_then_bounded_resubmission_is_durable_once() {
    withdrawal::fenced_epoch_bump_serializes_first_attempt_then_bounded_resubmission_is_durable_once();
}

#[test]
fn withdrawal_receipt_parser_bounds_shape_and_size() {
    withdrawal::withdrawal_receipt_parser_bounds_shape_and_size();
}

#[test]
fn withdrawal_retry_classifier_accepts_only_bounded_serialization_rejection() {
    withdrawal::withdrawal_retry_classifier_accepts_only_bounded_serialization_rejection();
}
