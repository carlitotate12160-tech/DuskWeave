//! Regression tests for M1 heartbeat lease coordination and authority fencing.
use super::fixture::*;
use super::lease_coordination_support::*;
use super::receipt::LabBasis;
use super::run_support::*;
use duskweave::m1_effect_session::{EffectPhase, EffectStore};
use duskweave::m1_reasoning::{
    inference::{self, Broker, ReleaseFailure},
    postgres::{Basis, PgDecisionStore},
    postgres_terrain::TerrainBasis,
    request,
};
use duskweave::m1_run::RunPhase;
use duskweave::postgres_m1_effect::PgEffectStore;
use duskweave::postgres_m1_run::PgRunStore;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_lease_renewal_does_not_own_release_fence() {
    assert!(worker());

    let (e, c, _operator) = register_live(0x7c01, false);
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
    let prepared = prepare_decision_one(&mut runs, &record);
    let deadline = Instant::now() + Duration::from_millis(prepared.window_ms as u64);
    let pack = prepared.pack.utf8().as_bytes();
    let base_port = broker_port(&record, deadline);

    let holder_client = broker();
    let mut decorated_port = HolderBroker::new(base_port, run, holder_client, Uuid::nil());

    let call_count = Arc::new(AtomicUsize::new(0));
    let release_count = Arc::new(AtomicUsize::new(0));
    let mut provider = CountingRepairProvider {
        count: call_count.clone(),
        releases: release_count.clone(),
    };

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
        &mut decorated_port,
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

    assert!(
        decorated_port.renew_executed,
        "run_renew must be executed during repair may_send"
    );
    assert!(
        !decorated_port.fence_owned_during_renew,
        "valid lease renewal must not acquire the campaign authority fence on schema 0034"
    );

    assert_eq!(
        call_count.load(Ordering::SeqCst),
        2,
        "generation and repair"
    );
    assert_eq!(
        release_count.load(Ordering::SeqCst),
        2,
        "both calls released"
    );
    assert_eq!(receipt.proposal_disposition, "schema_repair_exhausted");

    let decision_row: String = admin()
        .query_one(
            "SELECT payload->>'disposition' FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2 AND d=$3",
            &[&e, &c, &prepared.scope.decision],
        )
        .unwrap()
        .get(0);
    assert_eq!(decision_row, "schema_repair_exhausted");

    // Active LAB session proves valid renewal holds no campaign advisory lock.
    // Lifecycle: complete D1, advance/claim Acquisition, reserve through PgEffectStore.
    let (lab_e, lab_c, _) = register_live(0x7c02, true);
    let lab_run = Uuid::new_v4();
    let lab_pub = live_publication(lab_e, lab_c, lab_run);
    seed_live_qualification(&lab_pub);
    let mut lab_runs = PgRunStore::new(broker());
    lab_runs
        .start(
            &successor_request(&lab_pub, lab_run, "m1_autonomous_run_request_v4"),
            RELEASE,
        )
        .unwrap();
    let lab_record = lab_runs.inspect(lab_run).unwrap();

    let lab_prep = prepare_decision_one(&mut lab_runs, &lab_record);
    let lab_dead = Instant::now() + Duration::from_millis(lab_prep.window_ms as u64);
    let lab_pack = lab_prep.pack.utf8().as_bytes();
    let mut lab_provider = super::receipt::LiveProvider::default();
    let mut lab_port = broker_port(&lab_record, lab_dead);
    let mut lab_basis = LabBasis(TerrainBasis {
        mission: Basis {
            client: broker(),
            scope: lab_prep.scope.clone(),
            references: request::reference_ids(lab_pack).unwrap(),
            deadline: lab_dead,
        },
        dependencies: lab_prep.pack.terrain_dependencies().to_vec(),
    });
    let lab_receipt = inference::infer(
        &mut lab_port,
        &mut lab_provider,
        &mut lab_basis,
        &mut PgDecisionStore {
            client: client("DW_COGNITION_DATABASE_URL"),
            input_hash: lab_prep.input_hash.clone(),
        },
        &mut duskweave::postgres_trajectory::PgTrajectory::new(client(
            "DW_TRAJECTORY_DATABASE_URL",
        )),
        inference::InferenceInput {
            scope: &lab_prep.scope,
            pack: lab_pack,
            event_id: Uuid::new_v4(),
            occurred_at_ms: now_ms() as u64,
            source_mode: "blind".into(),
        },
    );
    assert_eq!(lab_receipt.publication, "committed");
    assert_eq!(lab_receipt.history, "completed");
    lab_runs
        .complete_phase(lab_run, RunPhase::DecisionOne)
        .unwrap();
    lab_runs
        .start_phase(lab_run, RunPhase::Acquisition)
        .unwrap();
    lab_runs
        .claim_phase(lab_run, RunPhase::Acquisition, RELEASE)
        .unwrap();

    let mut store = PgEffectStore::new(broker());
    let session = store.reserve(lab_run).unwrap();
    assert_eq!(session.phase, EffectPhase::Reserved);
    assert_eq!(session.run_id, lab_run);

    let eff_phase: String = admin()
        .query_one(
            "SELECT phase FROM execution.live_effect_control WHERE run_id=$1",
            &[&lab_run],
        )
        .unwrap()
        .get(0);
    assert_eq!(eff_phase, "reserved");

    let mut lab_holder = broker();
    lab_holder.batch_execute("BEGIN").unwrap();
    lab_holder
        .execute("SELECT execution.run_renew($1)", &[&lab_run])
        .unwrap();
    let lab_pid: i32 = lab_holder
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    let lab_key = fence_key(lab_e, lab_c);
    assert!(
        !holder_holds_fence(lab_pid, lab_key),
        "valid LAB renewal must not hold campaign advisory lock even during active live_effect_session"
    );
    lab_holder.batch_execute("COMMIT").unwrap();
}

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_lease_coordination_rejects_invalid_renewals() {
    assert!(worker());

    let (e, c, _) = register_live(0x7c03, false);
    let run = Uuid::new_v4();
    let publ = live_publication(e, c, run);
    let mut runs = PgRunStore::new(broker());
    runs.start(
        &successor_request(&publ, run, "m1_autonomous_run_request_v3"),
        RELEASE,
    )
    .unwrap();

    let (exp_e, exp_c, _) = register_live(0x7c33, false);
    let run_exp = Uuid::new_v4();
    let publ_exp = live_publication(exp_e, exp_c, run_exp);
    let mut runs_exp = PgRunStore::new(broker());
    runs_exp
        .start(
            &successor_request(&publ_exp, run_exp, "m1_autonomous_run_request_v3"),
            RELEASE,
        )
        .unwrap();

    let broker_role: String = broker()
        .query_one("SELECT session_user", &[])
        .unwrap()
        .get(0);
    let other_role: String = other_broker()
        .query_one("SELECT session_user", &[])
        .unwrap()
        .get(0);
    let terrain_role: String = terrain_column()
        .query_one("SELECT session_user", &[])
        .unwrap()
        .get(0);

    setup_test_mutator(&[run, run_exp], &[&broker_role, &other_role, &terrain_role]);

    // Reject out-of-scope run
    let out_of_scope = Uuid::new_v4();
    let err = broker()
        .execute(
            "SELECT execution.test_mutate_lease($1, $2, NULL)",
            &[&out_of_scope, &(now_ms() + 1000)],
        )
        .unwrap_err();
    assert!(
        db_message(&err).contains("test_mutator_scope_refused"),
        "out of scope run rejected: {err:?}"
    );

    let query_control = |r: Uuid| -> (i64, String) {
        let row = admin()
            .query_one(
                "SELECT lease_until_ms, status FROM execution.autonomous_control WHERE run_id=$1",
                &[&r],
            )
            .unwrap();
        (row.get(0), row.get(1))
    };

    let assert_refused = |client: &mut postgres::Client, r: Uuid, val: i64, desc: &str| {
        let before = query_control(r);
        let err = client
            .execute(
                "SELECT execution.test_mutate_lease($1, $2, NULL)",
                &[&r, &val],
            )
            .unwrap_err();
        assert!(
            db_message(&err).contains("run_not_current"),
            "{desc} must fail with run_not_current: {err:?}"
        );
        assert_eq!(
            query_control(r),
            before,
            "{desc}: row must remain unchanged after refusal"
        );
    };

    // 1. Regressing lease candidate in the trigger itself
    assert_refused(
        &mut broker(),
        run,
        query_control(run).0 - 100,
        "regressing lease",
    );

    // 2. Oversized positive lease candidate (> now + 2000) in the trigger itself
    assert_refused(&mut broker(), run, now_ms() + 10000, "oversized lease");

    // 3. Foreign caller in the trigger itself (not writer_oid of the run)
    assert_refused(&mut other_broker(), run, now_ms() + 1500, "foreign broker");

    // 4. Unqualified caller (not member of dw_m1_broker) in the trigger itself
    assert_refused(
        &mut terrain_column(),
        run,
        now_ms() + 1500,
        "unqualified caller",
    );

    // 5. Terminal state (actual STOP) in the trigger itself
    broker()
        .execute("SELECT execution.run_stop($1)", &[&run])
        .unwrap();
    let before_stopped = query_control(run);
    assert_eq!(
        (before_stopped.0, before_stopped.1.as_str()),
        (0, "stopped")
    );
    assert_refused(&mut broker(), run, now_ms() + 1500, "stopped run");

    // 6. Expired state in the trigger itself (real expiry observed on DB clock)
    let before_exp = query_control(run_exp);
    let mut exp_conn = broker();
    let start_exp = Instant::now();
    let mut is_past = false;
    while start_exp.elapsed() < Duration::from_secs(6) {
        let past: bool = exp_conn
            .query_one(
                "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint > $1",
                &[&before_exp.0],
            )
            .unwrap()
            .get(0);
        if past {
            is_past = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        is_past,
        "database clock must exceed expired run's initial lease"
    );
    assert_refused(&mut broker(), run_exp, now_ms() + 1500, "expired run");
}

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_authority_mutations_still_fence_release() {
    assert!(worker());

    let (e, c, _) = register_live(0x7c04, false);
    let run = Uuid::new_v4();
    let publ = live_publication(e, c, run);
    let mut runs = PgRunStore::new(broker());
    runs.start(
        &successor_request(&publ, run, "m1_autonomous_run_request_v3"),
        RELEASE,
    )
    .unwrap();
    let record = runs.inspect(run).unwrap();
    let prepared = prepare_decision_one(&mut runs, &record);
    let deadline = Instant::now() + Duration::from_millis(prepared.window_ms as u64);
    let pack = prepared.pack.utf8().as_bytes();
    let mut port = broker_port(&record, deadline);
    let reservation = port.reserve(&prepared.scope, pack).unwrap();
    let bound = duskweave::m1_reasoning::inference_intent::bind(
        &prepared.scope,
        &reservation,
        reservation.generation_call,
        pack,
        None,
    )
    .unwrap();
    port.may_send(&prepared.scope, reservation.generation_call, &bound)
        .unwrap();

    let key = fence_key(e, c);
    // Hold an actual run-owner STOP transaction reaching the mutation guard.
    let mut auth_tx = broker();
    auth_tx.batch_execute("BEGIN").unwrap();
    auth_tx
        .execute("SELECT execution.run_stop($1)", &[&run])
        .unwrap();
    let pid: i32 = auth_tx
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    assert!(
        holder_holds_fence(pid, key),
        "run-owner STOP transaction must hold the campaign advisory fence"
    );

    let call_count = Arc::new(AtomicUsize::new(0));
    let mut basis = LabBasis(TerrainBasis {
        mission: Basis {
            client: broker(),
            scope: prepared.scope.clone(),
            references: request::reference_ids(pack).unwrap(),
            deadline,
        },
        dependencies: prepared.pack.terrain_dependencies().to_vec(),
    });

    let result = port.release_into(
        &prepared.scope,
        &reservation,
        reservation.generation_call,
        &bound,
        &mut ZeroProvider(call_count.clone()),
        &mut basis,
    );
    assert!(matches!(
        result,
        Err(ReleaseFailure::NeverReleased(
            "current_authority_unavailable"
        ))
    ));
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        0,
        "provider must not be called when authority fence is active"
    );

    auth_tx.batch_execute("COMMIT").unwrap();
    assert!(
        !holder_holds_fence(pid, key),
        "after commit, authority fence must be released"
    );

    let stopped_row = admin()
        .query_one(
            "SELECT status, lease_until_ms FROM execution.autonomous_control WHERE run_id=$1",
            &[&run],
        )
        .unwrap();
    assert_eq!(stopped_row.get::<_, String>(0), "stopped");
    assert_eq!(stopped_row.get::<_, i64>(1), 0);

    // Direct-lock consumer control: external advisory lock also fences release_into.
    assert_direct_lock_fences_release(0x7c44);
}
