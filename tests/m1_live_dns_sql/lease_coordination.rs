//! Regression tests for M1 heartbeat lease coordination and authority fencing.
use super::fixture::*;
use super::lease_coordination_support::*;
use super::receipt::LabBasis;
use super::run_support::*;
use duskweave::m1_reasoning::{
    inference::{self, Broker, Provider, Received, ReleaseFailure},
    postgres::{Basis, PgDecisionStore},
    postgres_terrain::TerrainBasis,
    proposal::ProposalBasis,
    request::{self, BoundCall},
};
use duskweave::postgres_m1_run::PgRunStore;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
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
        "generation and repair entered"
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

    // Active LAB SQL session proves valid renewal holds no campaign advisory lock
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
        "valid LAB renewal must not hold campaign advisory lock"
    );
    lab_holder.batch_execute("COMMIT").unwrap();
}

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_lease_coordination_rejects_invalid_renewals() {
    assert!(worker());

    setup_test_mutator();

    let (e, c, _) = register_live(0x7c03, false);
    let run = Uuid::new_v4();
    let publ = live_publication(e, c, run);
    let mut runs = PgRunStore::new(broker());
    runs.start(
        &successor_request(&publ, run, "m1_autonomous_run_request_v3"),
        RELEASE,
    )
    .unwrap();

    let init_row = admin()
        .query_one(
            "SELECT lease_until_ms, status FROM execution.autonomous_control WHERE run_id=$1",
            &[&run],
        )
        .unwrap();
    let init_lease: i64 = init_row.get(0);

    // 1. Regressing lease candidate
    let err = broker()
        .execute(
            "SELECT execution.test_mutate_lease($1, $2, NULL)",
            &[&run, &(init_lease - 100)],
        )
        .unwrap_err();
    assert!(
        db_message(&err).contains("run_not_current"),
        "regressing lease must fail with run_not_current: {err:?}"
    );

    // 2. Oversized positive lease candidate (> now + 2000)
    let n = now_ms();
    let err = broker()
        .execute(
            "SELECT execution.test_mutate_lease($1, $2, NULL)",
            &[&run, &(n + 10000)],
        )
        .unwrap_err();
    assert!(
        db_message(&err).contains("run_not_current"),
        "oversized lease must fail with run_not_current: {err:?}"
    );

    // 3. Foreign/unqualified caller (not the writer_oid of the run)
    let err = other_broker()
        .execute("SELECT execution.run_renew($1)", &[&run])
        .unwrap_err();
    assert!(
        db_message(&err).contains("run_not_current"),
        "foreign broker must fail with run_not_current: {err:?}"
    );

    // 4. Terminal state (status='stopped')
    admin()
        .execute(
            "UPDATE execution.autonomous_control SET status='stopped', lease_until_ms=0 WHERE run_id=$1",
            &[&run],
        )
        .unwrap();
    let err = broker()
        .execute("SELECT execution.run_renew($1)", &[&run])
        .unwrap_err();
    assert!(
        db_message(&err).contains("run_not_current"),
        "renewal on stopped run must fail with run_not_current: {err:?}"
    );
}

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_authority_mutations_still_fence_release() {
    assert!(worker());

    struct ZeroProvider(Arc<AtomicUsize>);
    impl Provider for ZeroProvider {
        fn prepare(&mut self, _: &BoundCall, _: Instant) -> Result<(), &'static str> {
            Ok(())
        }
        fn call(
            &mut self,
            _: &BoundCall,
            _: Instant,
            _: &mut dyn ProposalBasis,
            release: &mut dyn FnMut() -> Result<(), &'static str>,
        ) -> Result<Received, &'static str> {
            self.0.fetch_add(1, Ordering::SeqCst);
            release()?;
            Err("provider_error")
        }
    }

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
    let mut auth_tx = admin();
    auth_tx.batch_execute("BEGIN").unwrap();
    auth_tx
        .execute("SELECT pg_advisory_xact_lock($1)", &[&key])
        .unwrap();
    let pid: i32 = auth_tx
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    assert!(
        holder_holds_fence(pid, key),
        "authority transaction must hold the campaign advisory fence"
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
}

#[test]
#[ignore = "runs only inside the dedicated restricted SQL fixture worker"]
fn sql_live_lease_renewal_preserves_stop_and_expiry() {
    assert!(worker());

    let (e, c, _) = register_live(0x7c05, false);
    let run = Uuid::new_v4();
    let publ = live_publication(e, c, run);
    let mut runs = PgRunStore::new(broker());
    runs.start(
        &successor_request(&publ, run, "m1_autonomous_run_request_v3"),
        RELEASE,
    )
    .unwrap();

    // 1. Concurrent renewals: transaction A holds row lock, transaction B waits
    let mut tx_a = broker();
    tx_a.batch_execute("BEGIN").unwrap();
    tx_a.execute("SELECT execution.run_renew($1)", &[&run])
        .unwrap();

    let b_unblocked = Arc::new(AtomicBool::new(false));
    let b_flag = b_unblocked.clone();
    let handle = thread::spawn(move || {
        let mut b_conn = broker();
        b_conn
            .execute("SELECT execution.run_renew($1)", &[&run])
            .unwrap();
        b_flag.store(true, Ordering::SeqCst);
    });

    thread::sleep(Duration::from_millis(100));
    assert!(
        !b_unblocked.load(Ordering::SeqCst),
        "tx B must block on row lock held by tx A"
    );

    tx_a.batch_execute("COMMIT").unwrap();
    handle.join().unwrap();
    assert!(
        b_unblocked.load(Ordering::SeqCst),
        "tx B completes after tx A commits"
    );

    // 2. STOP race: when run is stopped, renewal cannot resurrect it
    admin()
        .execute(
            "UPDATE execution.autonomous_control SET status='stopped', lease_until_ms=0 WHERE run_id=$1",
            &[&run],
        )
        .unwrap();

    let err = broker()
        .execute("SELECT execution.run_renew($1)", &[&run])
        .unwrap_err();
    assert!(
        db_message(&err).contains("run_not_current"),
        "run_renew on stopped run must fail with run_not_current"
    );

    let status: String = admin()
        .query_one(
            "SELECT status FROM execution.autonomous_control WHERE run_id=$1",
            &[&run],
        )
        .unwrap()
        .get(0);
    assert_eq!(
        status, "stopped",
        "status must remain stopped (no resurrection)"
    );
}
