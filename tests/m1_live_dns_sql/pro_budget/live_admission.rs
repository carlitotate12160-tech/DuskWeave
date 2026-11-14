//! Schema-0035 LIVE release-consent binding and current-inference qualification
//! tests against `execution.run_start` and `execution.inference_window_ms`.
use super::super::fixture::*;
use super::super::receipt::LabBasis;
use super::super::run_support::*;
use super::super::successor_episode::cli;
use super::live_binding::{
    LiveProV2SyntheticProvider, effect_and_run_counts, register_live_pro_v2,
};
use super::{PRO_ENDPOINT, register_pro_v2};
use duskweave::Fail;
use duskweave::m1_inference_profile as profiles;
use duskweave::m1_reasoning::{
    inference::{self, Broker, Decision, DecisionStore, Provider, ReleaseFailure, Reservation},
    inference_intent,
    postgres::{Basis, PgBroker, PgDecisionStore},
    postgres_terrain::TerrainBasis,
    proposal::ProposalBasis,
    request::{self, BoundCall},
};
use duskweave::m1_run::RunPhase;
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::postgres_trajectory::PgTrajectory;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use uuid::Uuid;

struct PreparedLiveV6 {
    e: Uuid,
    c: Uuid,
    qual_ref: Uuid,
    run: Uuid,
    runs: PgRunStore,
    record: duskweave::m1_run::RunRecord,
    prep: PreparedDecision,
}

fn prepare_live_v6(slot: u128, valid_until_ms: Option<i64>) -> PreparedLiveV6 {
    let (e, c, _) = register_live_pro_v2(
        slot,
        profiles::LIVE_RUN_PROFILE,
        profiles::PRO_V2_PROFILE,
        PRO_ENDPOINT,
        "9.9.9.9",
    );
    let run = Uuid::new_v4();
    let pub_val = live_publication(e, c, run);
    let qual_ref = seed_live_qualification(&pub_val);
    if let Some(until) = valid_until_ms {
        set_qual_until(qual_ref, until);
    }
    let mut runs = PgRunStore::new(broker());
    let req = successor_request(&pub_val, run, "m1_autonomous_run_request_v4");
    assert!(matches!(runs.start(&req, RELEASE), Ok(true)));
    let record = runs.inspect(run).unwrap();
    let prep = prepare_decision_one(&mut runs, &record);
    PreparedLiveV6 {
        e,
        c,
        qual_ref,
        run,
        runs,
        record,
        prep,
    }
}

fn cap_snapshot(e: Uuid, c: Uuid) -> Value {
    broker()
        .query_one(
            "SELECT execution.inference_capacity_snapshot($1,$2)",
            &[&e, &c],
        )
        .unwrap()
        .get(0)
}

fn fence_epoch(e: Uuid, c: Uuid) -> i64 {
    admin()
        .query_one(
            "SELECT epoch FROM execution.session_fences WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e, &c],
        )
        .unwrap()
        .get(0)
}

fn expire_qual(qual_ref: Uuid) {
    admin().execute("UPDATE execution.live_qualifications SET valid_from_ms = 1, valid_until_ms = 2 WHERE qualification_ref = $1", &[&qual_ref]).unwrap();
}
fn set_qual_until(qual_ref: Uuid, ms: i64) {
    admin().execute("UPDATE execution.live_qualifications SET valid_until_ms = $2 WHERE qualification_ref = $1", &[&qual_ref, &ms]).unwrap();
}
fn set_qual_release(qual_ref: Uuid, rel: &str) {
    admin().execute("UPDATE execution.live_qualifications SET release_sha256 = $2 WHERE qualification_ref = $1", &[&qual_ref, &rel]).unwrap();
}

fn assert_recovered_hold(s: &mut PreparedLiveV6, state: &str, holds: i64) {
    let snap = cap_snapshot(s.e, s.c);
    s.runs.stop(s.run).unwrap();
    let (ok_i, rec_i) = cli("m1-run-inspect", s.run);
    let (ok_r, rec_r) = cli("m1-run-recover", s.run);
    assert!(
        ok_i && ok_r
            && rec_i["inference"][0]["state"] == state
            && rec_r["inference"][0]["state"] == state
    );
    let after = cap_snapshot(s.e, s.c);
    assert_eq!(after["charged"], snap["charged"]);
    assert_eq!(after["active_holds"], holds);
}

fn win_bounds(e: Uuid, c: Uuid, d: Uuid) -> (i64, i64, i64, i64) {
    let row = admin()
        .query_one(
            "SELECT started_at_ms, active_until_ms, decision_until_ms, phase_until_ms FROM execution.inference_decision_windows WHERE (e,c,d)=($1,$2,$3)",
            &[&e, &c, &d],
        )
        .unwrap();
    (row.get(0), row.get(1), row.get(2), row.get(3))
}

fn make_basis(prep: &PreparedDecision, deadline: Instant) -> LabBasis {
    let pack = prep.pack.utf8().as_bytes();
    LabBasis(TerrainBasis {
        mission: Basis {
            client: broker(),
            scope: prep.scope.clone(),
            references: request::reference_ids(pack).unwrap(),
            deadline,
        },
        dependencies: prep.pack.terrain_dependencies().to_vec(),
    })
}

fn reserve_d1(s: &PreparedLiveV6) -> (PgBroker, Reservation, BoundCall, Instant) {
    let dl = Instant::now() + Duration::from_millis(s.prep.window_ms as u64);
    let pack = s.prep.pack.utf8().as_bytes();
    let mut port = broker_port(&s.record, dl);
    let res = port.reserve(&s.prep.scope, pack).unwrap();
    let bound =
        inference_intent::bind(&s.prep.scope, &res, res.generation_call, pack, None).unwrap();
    (port, res, bound, dl)
}

fn run_infer(
    s: &PreparedLiveV6,
    provider: &mut impl Provider,
    deadline: Instant,
    expire_before_publish: Option<Uuid>,
) -> inference::Receipt {
    let pack = s.prep.pack.utf8().as_bytes();
    let mut port = broker_port(&s.record, deadline);
    let mut basis = make_basis(&s.prep, deadline);
    let mut store = ExpireBeforePublishStore {
        expire_qual: expire_before_publish,
        inner: PgDecisionStore {
            client: client("DW_COGNITION_DATABASE_URL"),
            input_hash: s.prep.input_hash.clone(),
        },
    };
    inference::infer(
        &mut port,
        provider,
        &mut basis,
        &mut store,
        &mut PgTrajectory::new(client("DW_TRAJECTORY_DATABASE_URL")),
        inference::InferenceInput {
            scope: &s.prep.scope,
            pack,
            event_id: Uuid::new_v4(),
            occurred_at_ms: now_ms() as u64,
            source_mode: "blind".into(),
        },
    )
}

struct SendWitnessProvider {
    sends: Arc<AtomicUsize>,
}

impl Provider for SendWitnessProvider {
    fn prepare(&mut self, call: &BoundCall, _: Instant) -> Result<(), &'static str> {
        inference_intent::verify(call).map(|_| ())
    }

    fn call(
        &mut self,
        _: &BoundCall,
        _: Instant,
        _: &mut dyn ProposalBasis,
        _: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<inference::Received, &'static str> {
        self.sends.fetch_add(1, Ordering::SeqCst);
        Err("unexpected_provider_send")
    }
}

struct ExpireBeforePublishStore {
    expire_qual: Option<Uuid>,
    inner: PgDecisionStore,
}

impl DecisionStore for ExpireBeforePublishStore {
    fn publish_atomic(&mut self, decision: &Decision) -> Result<(), &'static str> {
        if let Some(qual_ref) = self.expire_qual {
            expire_qual(qual_ref);
        }
        self.inner.publish_atomic(decision)
    }

    fn inspect(&mut self, scope: &inference::Scope) -> Result<Option<Decision>, &'static str> {
        self.inner.inspect(scope)
    }
}

fn reg_live_case(slot: u128, live: bool) -> (Uuid, Uuid) {
    let (e, c, _) = if live {
        register_live_pro_v2(
            slot,
            profiles::LIVE_RUN_PROFILE,
            profiles::PRO_V2_PROFILE,
            PRO_ENDPOINT,
            "9.9.9.9",
        )
    } else {
        register_live(slot, true)
    };
    (e, c)
}

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_release_consent_matches_executing_release() {
    assert!(worker());
    assert_eq!(std::env::var_os("DEEPSEEK_API_KEY"), None);

    let release_b = "e".repeat(64);
    for (slot, is_live, label) in [(0xae01_u128, true, "LIVE"), (0xae02, false, "LAB")] {
        let (e, c) = reg_live_case(slot, is_live);
        let run = Uuid::new_v4();
        let pub_val = live_publication(e, c, run);
        let qual_ref = seed_live_qualification(&pub_val);
        set_qual_release(qual_ref, &release_b);
        let req = successor_request(&pub_val, run, "m1_autonomous_run_request_v4");
        assert_eq!(
            PgRunStore::new(broker()).start(&req, &release_b),
            Err(Fail::State("run_start_refused")),
            "{label}: consent releaseA != qual/exec releaseB must fail"
        );
        let req_json = serde_json::to_value(&req).unwrap();
        let sql_err = broker()
            .query_one(
                "SELECT execution.run_start($1,$2)",
                &[&req_json, &release_b],
            )
            .unwrap_err();
        assert!(
            db_message(&sql_err).contains("run_authority_unavailable"),
            "expected run_authority_unavailable, got {sql_err}"
        );
        assert_eq!(effect_and_run_counts(e), (0, 0, 0));
        let ctrl: i64 = admin()
            .query_one(
                "SELECT count(*) FROM execution.autonomous_control c JOIN execution.autonomous_runs r ON r.run_id=c.run_id WHERE r.e=$1",
                &[&e],
            )
            .unwrap()
            .get(0);
        assert_eq!(ctrl, 0);
    }

    for (slot, is_live) in [(0xae03_u128, true), (0xae04, false)] {
        let (e, c) = reg_live_case(slot, is_live);
        let run = Uuid::new_v4();
        let pub_val = live_publication(e, c, run);
        seed_live_qualification(&pub_val);
        let req = successor_request(&pub_val, run, "m1_autonomous_run_request_v4");
        assert!(matches!(
            PgRunStore::new(broker()).start(&req, RELEASE),
            Ok(true)
        ));
        assert_eq!(effect_and_run_counts(e), (1, 0, 0));
        PgRunStore::new(broker()).stop(run).unwrap();
    }
}

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_inference_qualification_is_current_at_marker_and_release() {
    assert!(worker());
    assert_eq!(std::env::var_os("DEEPSEEK_API_KEY"), None);

    // 1 & 5a. Valid v6 Pro-v2 bounded by qualification expiry without window refresh; completes D1.
    let mut bounded = prepare_live_v6(0xaf01, Some(now_ms() + 145_000));
    assert!(
        bounded.prep.window_ms > 122_000 && bounded.prep.window_ms <= 145_000,
        "window_ms={} must clamp to qualification expiry",
        bounded.prep.window_ms
    );
    let w_before = win_bounds(bounded.e, bounded.c, bounded.prep.scope.decision);
    let rem2: i64 = broker()
        .query_one(
            "SELECT execution.inference_window_ms($1,$2,$3)",
            &[&bounded.e, &bounded.c, &bounded.prep.scope.decision],
        )
        .unwrap()
        .get(0);
    assert!(rem2 > 0 && rem2 <= 145_000);
    let w_after = win_bounds(bounded.e, bounded.c, bounded.prep.scope.decision);
    assert_eq!(w_before, w_after, "window must never refresh or extend");
    let dl = Instant::now() + Duration::from_millis(bounded.prep.window_ms as u64);
    let receipt = run_infer(&bounded, &mut LiveProV2SyntheticProvider, dl, None);
    assert_eq!(receipt.proposal_disposition, "validated_proposal");
    bounded.runs.stop(bounded.run).unwrap();

    // 5b. V5 remains independent of live_qualifications.
    let (v5_e, v5_c, _) = register_pro_v2(0xaf02, profiles::PRO_V2_PROFILE);
    let v5_run = Uuid::new_v4();
    let v5_req = successor_request(
        &live_publication(v5_e, v5_c, v5_run),
        v5_run,
        "m1_autonomous_run_request_v3",
    );
    let mut v5_runs = PgRunStore::new(broker());
    assert!(matches!(v5_runs.start(&v5_req, RELEASE), Ok(true)));
    let v5_rec = v5_runs.inspect(v5_run).unwrap();
    let v5_prep = prepare_decision_one(&mut v5_runs, &v5_rec);
    assert!(v5_prep.window_ms > 230_000);
    v5_runs.stop(v5_run).unwrap();

    // 2. Post-start expiry, repudiation, or too-short qualification before may_send refuses cleanly.
    for (slot, mode) in [
        (0xaf05_u128, "expire"),
        (0xaf06, "repudiate"),
        (0xaf07, "too_short"),
    ] {
        let mut s = prepare_live_v6(slot, None);
        let (mut port, res, bound, _) = reserve_d1(&s);
        match mode {
            "expire" => expire_qual(s.qual_ref),
            "repudiate" => set_qual_release(s.qual_ref, &"c".repeat(64)),
            "too_short" => set_qual_until(s.qual_ref, now_ms() + 60_000),
            _ => unreachable!(),
        }
        s.runs
            .check(s.run)
            .expect("run_current remains independent of qualification");
        let epoch_before = fence_epoch(s.e, s.c);
        let snap_before = cap_snapshot(s.e, s.c);
        assert_eq!(
            port.may_send(&s.prep.scope, res.generation_call, &bound),
            Err("inference_marker_refused")
        );
        assert_eq!(
            fence_epoch(s.e, s.c),
            epoch_before,
            "{mode}: rejected may_send keeps epoch"
        );
        assert_eq!(
            cap_snapshot(s.e, s.c),
            snap_before,
            "{mode}: reservation retained"
        );
        let calls: i64 = admin()
            .query_one(
                "SELECT count(*) FROM execution.inference_calls WHERE e=$1",
                &[&s.e],
            )
            .unwrap()
            .get(0);
        assert_eq!(calls, 0);
        s.runs.stop(s.run).unwrap();
        let (ok, inspect_out) = cli("m1-run-inspect", s.run);
        assert!(ok && inspect_out["inference"].as_array().unwrap().is_empty());
    }

    // 3 & 4a. Expiry between successful may_send marker and final release refuses release_into with 0 sends;
    //         uncertain call stays charged and settles/recovers without HTTP.
    let mut mid = prepare_live_v6(0xaf08, None);
    let (mut port_mid, res_mid, bound_mid, dl_mid) = reserve_d1(&mid);
    port_mid
        .may_send(&mid.prep.scope, res_mid.generation_call, &bound_mid)
        .unwrap();
    expire_qual(mid.qual_ref);
    let rel_err = broker()
        .query_one(
            "SELECT execution.inference_release_current($1,$2,$3)",
            &[&mid.e, &mid.c, &mid.prep.scope.decision],
        )
        .unwrap_err();
    assert!(db_message(&rel_err).contains("live_qualification_unavailable"));
    let sends = Arc::new(AtomicUsize::new(0));
    let mut witness = SendWitnessProvider {
        sends: sends.clone(),
    };
    let mut basis_mid = make_basis(&mid.prep, dl_mid);
    assert!(matches!(
        port_mid.release_into(
            &mid.prep.scope,
            &res_mid,
            res_mid.generation_call,
            &bound_mid,
            &mut witness,
            &mut basis_mid
        ),
        Err(ReleaseFailure::NeverReleased(
            "current_authority_unavailable"
        ))
    ));
    assert_eq!(sends.load(Ordering::SeqCst), 0);
    assert_eq!(port_mid.inspect(&mid.prep.scope), Ok("uncertain_charged"));
    port_mid
        .uncertain_with_report(
            &mid.prep.scope,
            res_mid.generation_call,
            "provider_cancelled_uncertain",
            None,
        )
        .unwrap();
    assert_recovered_hold(&mut mid, "usage_unknown", 1);

    // 4b. Expiry after admitted release/call does not block DB-only settlement, history, or inspect/recover;
    //     subsequent D2 admission still blocks.
    let mut settled = prepare_live_v6(0xaf09, None);
    let (mut port_s, res_s, b_s, dl_s) = reserve_d1(&settled);
    port_s
        .may_send(&settled.prep.scope, res_s.generation_call, &b_s)
        .unwrap();
    let mut basis_s = make_basis(&settled.prep, dl_s);
    let rec_s = port_s
        .release_into(
            &settled.prep.scope,
            &res_s,
            res_s.generation_call,
            &b_s,
            &mut LiveProV2SyntheticProvider,
            &mut basis_s,
        )
        .ok()
        .unwrap();
    expire_qual(settled.qual_ref);
    port_s
        .completed_with_diagnostic(
            &settled.prep.scope,
            res_s.generation_call,
            &rec_s.usage,
            rec_s.admission.diagnostic().as_ref(),
        )
        .unwrap();
    port_s
        .close_unused_repair(&settled.prep.scope, &res_s)
        .unwrap();
    assert_eq!(
        port_s.inspect(&settled.prep.scope),
        Ok("completed_charged_unused_repair_reserved")
    );
    assert_recovered_hold(&mut settled, "completed", 1);

    let mut post = prepare_live_v6(0xaf0a, None);
    let dl_post = Instant::now() + Duration::from_millis(post.prep.window_ms as u64);
    let done = run_infer(
        &post,
        &mut LiveProV2SyntheticProvider,
        dl_post,
        Some(post.qual_ref),
    );
    assert_eq!(done.proposal_disposition, "validated_proposal");
    assert_eq!(done.publication, "committed");
    assert_eq!(done.history, "completed");
    post.runs
        .complete_phase(post.run, RunPhase::DecisionOne)
        .unwrap();
    let refill_pack = post.prep.pack.utf8().as_bytes();
    let mut d2_scope = post.prep.scope.clone();
    d2_scope.decision = post.record.decision_two;
    assert_eq!(
        broker_port(&post.record, dl_post)
            .reserve(&d2_scope, refill_pack)
            .err(),
        Some("inference_reservation_unknown")
    );
    let r2 = Uuid::new_v4();
    let r2_req = successor_request(
        &live_publication(post.e, post.c, r2),
        r2,
        "m1_autonomous_run_request_v4",
    );
    assert_eq!(
        post.runs.start(&r2_req, RELEASE),
        Err(Fail::State("run_start_refused"))
    );
    assert_recovered_hold(&mut post, "completed", 0);
    assert_eq!(effect_and_run_counts(post.e), (1, 1, 0));
}
