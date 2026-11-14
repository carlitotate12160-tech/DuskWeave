//! Production Pro-v2 intent, settlement, ledger and forgery rejection.
use super::super::fixture::*;
use super::super::receipt::LabBasis;
use super::super::run_support::*;
use super::{PRO_ENDPOINT, PRO_MODEL, register_pro_v2};
use duskweave::m1_context_capacity::ContextCapacity;
use duskweave::m1_inference_profile as profiles;
use duskweave::m1_reasoning::inference::{self, Broker, Provider, Received, Usage};
use duskweave::m1_reasoning::{
    inference_intent,
    postgres::{Basis, PgDecisionStore},
};
use duskweave::m1_reasoning::{
    postgres_terrain::TerrainBasis,
    proposal::ProposalBasis,
    request::{self, BoundCall},
    response,
};
use duskweave::m1_run::RunRecord;
use duskweave::postgres_m1_run::PgRunStore;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(super) struct CapturedCall {
    pub(super) config: Value,
    pub(super) digest: String,
    pub(super) call_id: Uuid,
}

#[derive(Default)]
pub(super) struct SyntheticRepairProProvider {
    pub(super) calls: usize,
    pub(super) captured: Vec<CapturedCall>,
}
impl Provider for SyntheticRepairProProvider {
    fn prepare(&mut self, call: &BoundCall, _: Instant) -> Result<(), &'static str> {
        let v = inference_intent::verify(call)?;
        assert_eq!(
            (
                &v["profile"],
                &v["recipient"],
                &v["model"],
                &v["timing_profile"],
                &v["settings"]["max_output_tokens"]
            ),
            (
                &json!(profiles::PRO_V2_PROFILE),
                &json!(PRO_ENDPOINT),
                &json!(PRO_MODEL),
                &json!(profiles::PRO_V2_TIMING_PROFILE),
                &json!(8192)
            )
        );
        Ok(())
    }
    fn call(
        &mut self,
        call: &BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        release()?;
        let config: Value = serde_json::from_slice(&call.configuration).map_err(|_| "cfg")?;
        let call_id = config["call_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or("id")?;
        self.captured.push(CapturedCall {
            config: config.clone(),
            digest: call.digest.clone(),
            call_id,
        });
        let mut proposal =
            genai::proposal(&serde_json::from_slice::<Value>(&call.pack).map_err(|_| "pack")?);
        let idx = self.calls;
        self.calls += 1;
        let (reported, i, o, r, b, w) = if idx == 0 {
            assert_eq!(config["call_role"], "generation");
            proposal.as_object_mut().unwrap().remove("hypothesis");
            ("probe-fixture", 111, 222, 77, 512, 4000)
        } else {
            assert_eq!(config["call_role"], "repair");
            assert!(!config["repair_error_codes"].as_array().unwrap().is_empty());
            ("deepseek-v4-pro", 200, 300, 99, 800, 5000)
        };
        let raw = serde_json::to_vec(&genai::envelope(&proposal)).map_err(|_| "encode")?;
        let mut rec = response::admit_envelope(&raw, raw.len(), Duration::ZERO, basis)?;
        rec.usage = Usage {
            input_tokens: Some(i),
            output_tokens: Some(o),
            reasoning_tokens: Some(r),
            response_bytes: b,
            wall_time_ms: w,
            requested_model: PRO_MODEL,
            reported_model: Some(reported.into()),
        };
        Ok(rec)
    }
}

pub(super) fn capacity_snapshot(e: Uuid, c: Uuid) -> Value {
    broker()
        .query_one(
            "SELECT execution.inference_capacity_snapshot($1,$2)",
            &[&e, &c],
        )
        .unwrap()
        .get(0)
}

pub(super) fn assert_counts(e: Uuid, c: Uuid, expected_res: i64, expected_calls: i64) {
    let r: i64 = admin()
        .query_one(
            "SELECT count(*) FROM execution.inference_reservations WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e, &c],
        )
        .unwrap()
        .get(0);
    let k: i64 = admin()
        .query_one(
            "SELECT count(*) FROM execution.inference_calls WHERE e=$1 AND c=$2",
            &[&e, &c],
        )
        .unwrap()
        .get(0);
    assert_eq!((r, k), (expected_res, expected_calls));
}

pub(super) fn start_d1(e: Uuid, c: Uuid) -> (Value, Uuid, PgRunStore, RunRecord, PreparedDecision) {
    let run = Uuid::new_v4();
    let publication = live_publication(e, c, run);
    let mut runs = PgRunStore::new(broker());
    assert!(matches!(
        runs.start(
            &successor_request(&publication, run, "m1_autonomous_run_request_v3"),
            RELEASE
        ),
        Ok(true)
    ));
    let record = runs.inspect(run).unwrap();
    let prep = prepare_decision_one(&mut runs, &record);
    (publication, run, runs, record, prep)
}

pub(super) fn run_prepared_inference<P: Provider>(
    record: &RunRecord,
    prep: &PreparedDecision,
    provider: &mut P,
) -> inference::Receipt {
    let dl = Instant::now() + Duration::from_millis(prep.window_ms as u64);
    let mut port = broker_port(record, dl);
    let mut basis = LabBasis(TerrainBasis {
        mission: Basis {
            client: broker(),
            scope: prep.scope.clone(),
            references: request::reference_ids(prep.pack.utf8().as_bytes()).unwrap(),
            deadline: dl,
        },
        dependencies: prep.pack.terrain_dependencies().to_vec(),
    });
    inference::infer(
        &mut port,
        provider,
        &mut basis,
        &mut PgDecisionStore {
            client: client("DW_COGNITION_DATABASE_URL"),
            input_hash: prep.input_hash.clone(),
        },
        &mut duskweave::postgres_trajectory::PgTrajectory::new(client(
            "DW_TRAJECTORY_DATABASE_URL",
        )),
        inference::InferenceInput {
            scope: &prep.scope,
            pack: prep.pack.utf8().as_bytes(),
            event_id: Uuid::new_v4(),
            occurred_at_ms: now_ms() as u64,
            source_mode: "blind".into(),
        },
    )
}

fn assert_forged_intents_rejected(slot: u128) {
    let (e, c, _) = register_pro_v2(slot, profiles::PRO_V2_PROFILE);
    let (_pub, run, mut runs, record, prep) = start_d1(e, c);
    let mut port = broker_port(
        &record,
        Instant::now() + Duration::from_millis(prep.window_ms as u64),
    );
    let pack = prep.pack.utf8().as_bytes();
    let res = port.reserve(&prep.scope, pack).unwrap();
    let gen_b = inference_intent::bind(&prep.scope, &res, res.generation_call, pack, None).unwrap();
    let rep_b = inference_intent::bind(
        &prep.scope,
        &res,
        res.repair_call,
        pack,
        Some(&["proposal_shape"]),
    )
    .unwrap();
    for (base, call_id, kind) in [
        (&gen_b, res.generation_call, 0),
        (&gen_b, res.generation_call, 1),
        (&gen_b, res.generation_call, 2),
        (&rep_b, res.repair_call, 2),
    ] {
        let mut cfg: Value = serde_json::from_slice(&base.configuration).unwrap();
        match kind {
            0 => cfg["profile"] = json!("fixture-genai-responses-v1"),
            1 => cfg["timing_profile"] = json!("reasoning-bounded-v1"),
            _ => {
                cfg["settings"]["max_output_tokens"] = json!(4096);
                cfg["limits"]["output_tokens_per_call"] = json!(4096);
            }
        }
        cfg.sort_all_objects();
        let bytes = serde_json::to_vec(&cfg).unwrap();
        let forged = BoundCall {
            pack: pack.to_vec(),
            configuration: bytes.clone(),
            body: Vec::new(),
            digest: inference_intent::digest(pack, &bytes),
        };
        assert_eq!(
            inference_intent::verify(&forged),
            Err("inference_decision_integrity")
        );
        assert_eq!(
            port.may_send(&prep.scope, call_id, &forged),
            Err("inference_marker_refused")
        );
    }
    assert_counts(e, c, 1, 0);
    runs.stop(run).unwrap();
}

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_pro_v2_production_profile_intent_settlement_and_ledger() {
    assert!(worker());
    assert_eq!(std::env::var_os("DEEPSEEK_API_KEY"), None);

    let (e, c, _) = register_pro_v2(0xab01, profiles::PRO_V2_PROFILE);
    let (pub_val, run, mut runs, record, prep) = start_d1(e, c);
    assert_eq!(
        pub_val["fields"]["m1_inference_permission"]["campaign_limits"],
        json!({"calls": 4, "request_bytes": 65536, "response_bytes": 262144, "output_token_capacity": 32768, "wall_time_ms": 498000})
    );
    assert!((243_000..=244_000).contains(&prep.window_ms));
    assert_eq!(
        serde_json::from_str::<Value>(prep.pack.utf8()).unwrap()["inference_profile"],
        profiles::PRO_V2_PROFILE
    );
    assert_eq!(capacity_snapshot(e, c)["charged"]["calls"], 0);

    let mut provider = SyntheticRepairProProvider::default();
    let receipt = run_prepared_inference(&record, &prep, &mut provider);
    assert_eq!(
        (
            receipt.proposal_disposition,
            receipt.publication,
            receipt.history,
            provider.calls
        ),
        ("validated_proposal", "committed", "completed", 2)
    );

    let res_row = admin()
        .query_one(
            "SELECT generation, generation_call_id, repair_call_id, capacity FROM execution.inference_reservations WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3 AND record_revision=1",
            &[&e, &c, &prep.scope.decision],
        )
        .unwrap();
    let (generation, gen_id, rep_id, res_cap): (i64, Uuid, Uuid, Value) = (
        res_row.get(0),
        res_row.get(1),
        res_row.get(2),
        res_row.get(3),
    );
    assert_eq!(
        res_cap,
        json!({"calls": 2, "request_bytes": 32768, "response_bytes": 131072, "output_token_capacity": 16384, "wall_time_ms": 249000})
    );
    assert_eq!(
        (
            provider.captured[0].call_id,
            provider.captured[1].call_id,
            provider.captured[0].config["generation"].as_i64()
        ),
        (gen_id, rep_id, Some(generation))
    );

    let snap = capacity_snapshot(e, c);
    assert_eq!(snap["charged"], res_cap);
    let rem = serde_json::from_value::<ContextCapacity>(snap)
        .unwrap()
        .remaining([4, 65536, 262144, 32768, 498000])
        .unwrap();
    assert_eq!(
        (
            rem.calls,
            rem.request_bytes,
            rem.response_bytes,
            rem.output_token_capacity,
            rem.wall_time_ms
        ),
        (2, 32768, 131072, 16384, 249000)
    );

    let calls = admin()
        .query(
            "SELECT call, binding, state, configuration::jsonb, usage FROM execution.inference_calls WHERE e=$1 AND c=$2 AND d=$3 ORDER BY configuration::jsonb->>'call_role'",
            &[&e, &c, &prep.scope.decision],
        )
        .unwrap();
    assert_eq!(calls.len(), 2);
    for (row, role, reported) in [
        (&calls[0], "generation", "probe-fixture"),
        (&calls[1], "repair", "deepseek-v4-pro"),
    ] {
        let cap = provider
            .captured
            .iter()
            .find(|x| x.call_id == row.get::<_, Uuid>(0))
            .unwrap();
        let (cfg, usage): (Value, Value) = (row.get(3), row.get(4));
        assert_eq!(
            (row.get::<_, String>(1), row.get::<_, String>(2)),
            (cap.digest.clone(), "completed".into())
        );
        assert_eq!(
            (
                &cfg["call_role"],
                &cfg["profile"],
                &cfg["recipient"],
                &cfg["model"],
                &cfg["timing_profile"]
            ),
            (
                &json!(role),
                &json!(profiles::PRO_V2_PROFILE),
                &json!(PRO_ENDPOINT),
                &json!(PRO_MODEL),
                &json!(profiles::PRO_V2_TIMING_PROFILE)
            )
        );
        assert_eq!(
            (&usage["requested_model"], &usage["reported_model"]),
            (&json!(PRO_MODEL), &json!(reported))
        );
    }
    let dec: Value = admin()
        .query_one(
            "SELECT payload FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2 AND d=$3",
            &[&e, &c, &prep.scope.decision],
        )
        .unwrap()
        .get(0);
    assert_eq!(dec["disposition"], "validated_proposal");
    let models: Vec<_> = dec["provenance"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["reported"]["requested_model"].as_str(),
                p["reported"]["reported_model"].as_str(),
            )
        })
        .collect();
    assert_eq!(models.len(), 2);
    assert!(
        models.contains(&(Some(PRO_MODEL), Some("probe-fixture")))
            && models.contains(&(Some(PRO_MODEL), Some("deepseek-v4-pro")))
    );
    runs.stop(run).unwrap();
    assert_forged_intents_rejected(0xab02);
}

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_pro_v2_capacity_exhaustion_and_unknown_charge() {
    assert!(worker());
    assert_eq!(std::env::var_os("DEEPSEEK_API_KEY"), None);
    super::capacity::run_capacity_and_unknown_suite();
}
