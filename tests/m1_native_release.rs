//! Deterministic release barriers against the ordinary legacy Mission SQL writer.
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_native_inference.rs"]
mod native;
use duskweave::m1_reasoning::{inference::*, postgres::Basis, proposal::ProposalBasis, request};
use duskweave::mission::*;
use duskweave::registration::OperationAllocator;
use duskweave::withdrawal::{MissionAuthorityWithdrawn, WithdrawalReason, WithdrawalRequest};
use postgres::GenericClient;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc::{Receiver, Sender, channel},
};
use std::time::{Duration, Instant};

fn stop(c: &mut impl GenericClient, s: &Scope) {
    let row = c.query_one("SELECT operation_id,floor(extract(epoch FROM clock_timestamp()))::bigint FROM mission.missions WHERE engagement_id=$1 AND campaign_id=$2",&[&s.engagement,&s.campaign]).unwrap();
    let mut allocator =
        duskweave::postgres_mission::PgAllocator::new(native::client("DW_TEST_DATABASE_URL"));
    let op = OperationId(allocator.allocate().unwrap());
    let event = EventId(allocator.allocate().unwrap());
    let withdrawal = MissionAuthorityWithdrawn::new(
        WithdrawalRequest {
            engagement_id: EngagementId(s.engagement),
            campaign_id: CampaignId(s.campaign),
            operator_ref: OperatorRef(uuid::Uuid::from_u128(0x11)),
            expected_mission_revision: 1,
            reason: WithdrawalReason::OperatorRequested,
        },
        op,
        OperationId(row.get(0)),
        event,
        row.get(1),
    )
    .unwrap();
    c.execute("INSERT INTO mission.withdrawals(engagement_id,campaign_id,operation_id,event_id,registration_operation_id,contract) VALUES($1,$2,$3,$4,$5,$6)",&[&s.engagement,&s.campaign,&op.0,&event.0,&withdrawal.registration_operation_id.0,&serde_json::to_value(&withdrawal).unwrap()]).unwrap();
}

struct CountingProvider(Arc<AtomicUsize>);
impl Provider for CountingProvider {
    fn prepare(&mut self, _: &request::BoundCall, _: Instant) -> Result<(), &'static str> {
        Ok(())
    }
    fn call(
        &mut self,
        _: &request::BoundCall,
        _: Instant,
        _: &mut dyn ProposalBasis,
        _: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err("provider_release_refused")
    }
}

struct BarrierProvider {
    launched: Arc<AtomicUsize>,
    ready: Sender<()>,
    finish: Receiver<()>,
}
impl Provider for BarrierProvider {
    fn prepare(&mut self, _: &request::BoundCall, _: Instant) -> Result<(), &'static str> {
        Ok(())
    }
    fn call(
        &mut self,
        _: &request::BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        std::thread::scope(|scope| {
            let launched = &self.launched;
            let ready = &self.ready;
            let finish = &mut self.finish;
            let worker = scope.spawn(move || {
                launched.fetch_add(1, Ordering::SeqCst);
                ready.send(()).unwrap();
                finish.recv_timeout(Duration::from_secs(5)).unwrap();
                assert!(
                    basis.current_authority_and_premises().is_err(),
                    "direct STOP is visible without history delivery"
                );
                Err("provider_cancelled_uncertain")
            });
            let opened = release();
            let result = worker.join().unwrap();
            opened?;
            result
        })
    }
}

fn basis(case: &native::NativeCase) -> Basis {
    Basis {
        client: native::client("DW_INFERENCE_BROKER_DATABASE_URL"),
        scope: case.scope.clone(),
        references: request::reference_ids(&case.pack).unwrap(),
        deadline: case.broker.deadline,
    }
}

#[test]
fn direct_stop_wins_before_launch_and_prevents_any_provider_call() {
    let _guard = native::db();
    let mut case = native::decision(0x9910);
    let r = case.broker.reserve(&case.scope, &case.pack).unwrap();
    let bound = request::bind(&case.pack, None).unwrap();
    case.broker
        .may_send(&case.scope, r.generation_call, &bound)
        .unwrap();
    let mut b = basis(&case);
    let mut legacy = native::client("DW_TEST_DATABASE_URL");
    let mut tx = legacy.transaction().unwrap();
    stop(&mut tx, &case.scope); // uncommitted writer trigger holds the campaign fence
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let (ready, _) = channel();
    let (_, finish) = channel();
    let scope_id = case.scope.clone();
    let (done, completed) = channel();
    let worker = std::thread::spawn(move || {
        let result = case.broker.release_into(
            &case.scope,
            &r,
            r.generation_call,
            &bound,
            &mut BarrierProvider {
                launched: observed,
                ready,
                finish,
            },
            &mut b,
        );
        let _ = done.send(());
        (case.broker, result)
    });
    let refused = completed.recv_timeout(Duration::from_secs(3)).is_ok();
    tx.commit().unwrap(); // always release the held fence before collecting
    let (_, result) = worker.join().unwrap();
    assert!(
        refused,
        "release must refuse promptly instead of waiting on the held fence"
    );
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert!(matches!(
        result,
        Err(ReleaseFailure::NeverReleased(
            "current_authority_unavailable"
        ))
    ));
    assert_eq!(native::admin().query_one("SELECT count(*) FROM mission.withdrawals WHERE engagement_id=$1 AND campaign_id=$2",&[&scope_id.engagement,&scope_id.campaign]).unwrap().get::<_,i64>(0),1);
}

#[test]
fn held_campaign_fence_refuses_release_without_waiting_or_provider_call() {
    let _guard = native::db();
    let mut case = native::decision(0x9912);
    case.broker.deadline = Instant::now() + Duration::from_secs(4);
    let r = case.broker.reserve(&case.scope, &case.pack).unwrap();
    let bound = request::bind(&case.pack, None).unwrap();
    case.broker
        .may_send(&case.scope, r.generation_call, &bound)
        .unwrap();
    let mut b = basis(&case);
    let s = case.scope.clone();
    let key = format!("{}:{}", s.engagement, s.campaign);
    let mut holder = native::client("DW_TEST_DATABASE_URL");
    holder
        .batch_execute("SET statement_timeout=3000; SET lock_timeout=2000")
        .unwrap();
    holder
        .query_one("SELECT pg_advisory_lock(hashtextextended($1,0))", &[&key])
        .unwrap();
    let pid: i32 = case
        .broker
        .client
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let call_id = r.generation_call;
    let (done, completed) = channel();
    let worker = std::thread::spawn(move || {
        let result = case.broker.release_into(
            &case.scope,
            &r,
            call_id,
            &bound,
            &mut CountingProvider(observed),
            &mut b,
        );
        let _ = done.send(());
        (case.broker, result)
    });
    let refused = completed.recv_timeout(Duration::from_secs(3)).is_ok();
    // Owned bounded cleanup of the held fence precedes worker collection.
    holder
        .query_one("SELECT pg_advisory_unlock_all()", &[])
        .unwrap();
    let (mut broker, result) = worker.join().unwrap();
    assert!(
        refused,
        "release must refuse promptly instead of waiting for the held fence"
    );
    assert!(
        matches!(
            result,
            Err(ReleaseFailure::NeverReleased(
                "current_authority_unavailable"
            ))
        ),
        "fence contention must refuse without invoking the provider"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let leaked: i64 = native::admin()
        .query_one(
            "SELECT count(*) FROM pg_locks WHERE pid=$1 AND locktype='advisory'",
            &[&pid],
        )
        .unwrap()
        .get(0);
    assert_eq!(leaked, 0, "a refused try-lock owns no session fence");
    broker
        .never_released(&s, call_id, "current_authority_unavailable")
        .unwrap();
    assert_eq!(
        broker.inspect(&s).unwrap(),
        "failed_never_released_reserved"
    );
}

struct ExpiringBasis {
    inner: Basis,
    shared_deadline: Instant,
}
impl ProposalBasis for ExpiringBasis {
    fn reference_eligible(&self, r: &str) -> bool {
        self.inner.reference_eligible(r)
    }
    fn manifest_eligible(&self, r: &str) -> bool {
        self.inner.manifest_eligible(r)
    }
    fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
        self.inner.current_authority_and_premises()
    }
    fn before_provider_release(&mut self) -> Result<(), &'static str> {
        // Bounded barrier: validation completes only after the shared deadline.
        while Instant::now() < self.shared_deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        self.inner.before_provider_release()
    }
    fn safe_explanation(&self, text: &str) -> bool {
        self.inner.safe_explanation(text)
    }
}

#[test]
fn deadline_expiring_during_validation_releases_the_fence_without_provider_call() {
    let _guard = native::db();
    let mut case = native::decision(0x9913);
    case.broker.deadline = Instant::now() + Duration::from_millis(500);
    let r = case.broker.reserve(&case.scope, &case.pack).unwrap();
    let bound = request::bind(&case.pack, None).unwrap();
    case.broker
        .may_send(&case.scope, r.generation_call, &bound)
        .unwrap();
    let mut b = ExpiringBasis {
        inner: Basis {
            client: native::client("DW_INFERENCE_BROKER_DATABASE_URL"),
            scope: case.scope.clone(),
            references: request::reference_ids(&case.pack).unwrap(),
            deadline: Instant::now() + Duration::from_secs(60),
        },
        shared_deadline: r.shared_deadline,
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let result = case.broker.release_into(
        &case.scope,
        &r,
        r.generation_call,
        &bound,
        &mut CountingProvider(calls.clone()),
        &mut b,
    );
    assert!(matches!(
        result,
        Err(ReleaseFailure::NeverReleased("inference_deadline"))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let mut other = native::client("DW_TEST_DATABASE_URL");
    let key = format!("{}:{}", case.scope.engagement, case.scope.campaign);
    let acquired: bool = other
        .query_one(
            "SELECT pg_try_advisory_lock(hashtextextended($1,0))",
            &[&key],
        )
        .unwrap()
        .get(0);
    assert!(acquired, "deadline refusal must release the acquired fence");
    other
        .query_one("SELECT pg_advisory_unlock_all()", &[])
        .unwrap();
}

#[test]
fn stop_after_owned_launch_commits_while_worker_waits_and_retains_uncertainty() {
    let _guard = native::db();
    let mut case = native::decision(0x9911);
    let r = case.broker.reserve(&case.scope, &case.pack).unwrap();
    let bound = request::bind(&case.pack, None).unwrap();
    case.broker
        .may_send(&case.scope, r.generation_call, &bound)
        .unwrap();
    let mut b = basis(&case);
    let s = case.scope.clone();
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let (ready, started) = channel();
    let (finish, done) = channel();
    let (mut broker, result) = std::thread::scope(|threads| {
        let worker = threads.spawn(move || {
            let result = case.broker.release_into(
                &case.scope,
                &r,
                r.generation_call,
                &bound,
                &mut BarrierProvider {
                    launched: observed,
                    ready,
                    finish: done,
                },
                &mut b,
            );
            (case.broker, result)
        });
        started
            .recv_timeout(Duration::from_secs(3))
            .expect("owned launch barrier");
        let mut legacy = native::client("DW_TEST_DATABASE_URL");
        legacy.batch_execute("SET statement_timeout=3000").unwrap();
        stop(&mut legacy, &s); // must commit before the blocked worker can finish
        finish.send(()).unwrap();
        worker.join().unwrap()
    });
    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "release already occurred; zero-send is not promised"
    );
    assert!(matches!(
        result,
        Err(ReleaseFailure::MayHaveSent("provider_cancelled_uncertain"))
    ));
    let call: uuid::Uuid = native::admin()
        .query_one(
            "SELECT call FROM execution.inference_calls WHERE d=$1",
            &[&s.decision],
        )
        .unwrap()
        .get(0);
    broker
        .uncertain(&s, call, "provider_cancelled_uncertain")
        .unwrap();
    assert_eq!(broker.inspect(&s).unwrap(), "uncertain_charged");
    assert_eq!(native::admin().query_one("SELECT count(*) FROM trajectory.withdrawal_history WHERE engagement_id=$1 AND campaign_id=$2",&[&s.engagement,&s.campaign]).unwrap().get::<_,i64>(0),0,"cancellation did not wait for normal history propagation");
}
