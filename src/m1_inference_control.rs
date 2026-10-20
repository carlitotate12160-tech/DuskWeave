//! One outstanding restricted authority/premise check; transport callbacks never invoke it.
use super::m1_run_control::Binding;
use duskweave::m1_reasoning::postgres_terrain::TerrainBasis;
use duskweave::m1_reasoning::proposal::ProposalBasis;
use duskweave::m1_reasoning::transport_control::Permit;
use duskweave::{Fail, Res};
use postgres::Client;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub(super) struct Coordinator {
    pub(super) permit: Permit,
    done: Sender<()>,
}
struct Authority {
    client: Client,
    terrain: TerrainBasis,
    binding: Binding,
    run: Uuid,
    deadline: Instant,
    last_start: Option<Instant>,
}
impl Authority {
    fn renew(&mut self, permit: &Permit) -> Res<Instant> {
        let started = Instant::now();
        if self
            .last_start
            .is_some_and(|last| started - last > Duration::from_millis(500))
        {
            return Err(Fail::State("inference_control_unavailable"));
        }
        self.last_start = Some(started);
        let valid_until = self.current_limit(started)?;
        if started.elapsed() > Duration::from_millis(500) || !permit.publish(started, valid_until) {
            return Err(Fail::State("inference_control_unavailable"));
        }
        Ok(started)
    }
    fn current_limit(&mut self, started: Instant) -> Res<Instant> {
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Fail::State("clock_unavailable"))?
            .as_millis() as i64;
        self.binding.current()?;
        let row = self
            .client
            .query_one(
                "SELECT execution.run_current($1),least(x.deadline_at_ms,y.lease_until_ms) \
             FROM execution.autonomous_runs x JOIN execution.autonomous_control y USING(run_id) \
             WHERE x.run_id=$1",
                &[&self.run],
            )
            .map_err(|_| Fail::State("current_authority_unavailable"))?;
        let until: i64 = row
            .try_get(1)
            .map_err(|_| Fail::State("current_authority_unavailable"))?;
        let until = self
            .terrain
            .dependencies
            .iter()
            .map(|d| d.valid_until.saturating_mul(1000))
            .fold(until, i64::min);
        self.terrain
            .before_provider_release()
            .map_err(Fail::State)?;
        let left = until
            .checked_sub(wall)
            .filter(|n| *n > 0)
            .ok_or(Fail::State("current_authority_unavailable"))?;
        Ok((started + Duration::from_millis(left as u64)).min(self.deadline))
    }
}
impl Coordinator {
    pub(super) fn start(
        mut client: Client,
        mut terrain: TerrainBasis,
        binding: Binding,
        run: Uuid,
        deadline: Instant,
    ) -> Res<Self> {
        for c in [&mut client, &mut terrain.mission.client] {
            c.batch_execute("SET statement_timeout='200ms'; SET lock_timeout='100ms'")
                .map_err(|_| Fail::Config("inference_control_unavailable"))?;
        }
        let permit = Permit::new(deadline);
        let mut authority = Authority {
            client,
            terrain,
            binding,
            run,
            deadline,
            last_start: None,
        };
        let first = authority.renew(&permit)?;
        super::m1_run_control::read_parent(permit.clone())?;
        let (done, rx) = channel();
        let owned = permit.clone();
        std::thread::Builder::new()
            .name("inference_authority_coordinator".into())
            .spawn(move || renewals(|p| authority.renew(p), owned, first, rx))
            .map_err(|_| Fail::Config("inference_control_unavailable"))?;
        Ok(Self { permit, done })
    }
}
fn renewals(
    mut check: impl FnMut(&Permit) -> Res<Instant>,
    permit: Permit,
    mut started: Instant,
    done: Receiver<()>,
) {
    loop {
        let next = started + Duration::from_millis(400);
        match done.recv_timeout(next.saturating_duration_since(Instant::now())) {
            Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
        match check(&permit) {
            Ok(at) => started = at,
            Err(_) => {
                permit.stop();
                return;
            }
        }
    }
}
impl Drop for Coordinator {
    fn drop(&mut self) {
        self.permit.stop();
        let _ = self.done.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn one_blocked_check_cannot_block_permit_reads_or_publish_a_late_success() {
        let until = Instant::now() + Duration::from_secs(10);
        let permit = Permit::new(until);
        let initial = Instant::now();
        assert!(permit.publish(initial, until));
        let owned = permit.clone();
        let (entered, observed) = channel();
        let (resume, parked) = channel();
        let (done, rx) = channel();
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let worker = std::thread::spawn(move || {
            renewals(
                |p| {
                    counter.fetch_add(1, Ordering::SeqCst);
                    let started = Instant::now();
                    entered.send(started).unwrap();
                    parked.recv().unwrap();
                    p.publish(started, until)
                        .then_some(started)
                        .ok_or(Fail::State("stale_check"))
                },
                owned,
                initial,
                rx,
            )
        });
        observed
            .recv_timeout(Duration::from_millis(600))
            .expect("400ms renewal start");
        let reads = Instant::now();
        for _ in 0..1000 {
            assert!(permit.current());
        }
        assert!(
            reads.elapsed() < Duration::from_millis(50),
            "callback-local reads waited for a check"
        );
        let end = initial + Duration::from_millis(2200);
        while permit.current() {
            assert!(Instant::now() < end, "stalled check extended the lease");
            std::thread::yield_now();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1, "no thread per renewal");
        resume.send(()).unwrap();
        worker.join().unwrap();
        assert!(permit.stopped());
        assert!(
            !permit.publish(Instant::now(), until),
            "a late check cannot revive an expired permit"
        );
        drop(done);
    }

    #[test]
    fn disarmed_coordinator_has_no_follow_on_check() {
        let permit = Permit::new(Instant::now() + Duration::from_secs(2));
        let (done, rx) = channel();
        done.send(()).unwrap();
        renewals(|_| panic!("disarmed"), permit, Instant::now(), rx);
    }
}
