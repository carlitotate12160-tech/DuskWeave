//! In-module fault and clock controls for the private wait-bound module.
//! Injected instants prove the cumulative budget state machine; channel
//! inspection proves the pause/resume controls around the C0 response read,
//! including the inactive handle without an installed command guard and
//! disconnected controls. The socket-policy assertions prove the consumed
//! configuration; real CLI termination bounds and server timeout behavior
//! are covered by the integration suites, not by these doubles.

use super::*;
use crate::authority_confirmation::confirm_if_new;
use duskweave::mission::{AssetRef, CampaignId, EngagementId, GoalRef, OperationId};
use duskweave::planning::PlanningRequest;
use duskweave::planning_assessment::PlanningStore;
use duskweave::registration::OperationAllocator;
use duskweave::{Fail, Res};
use std::sync::mpsc::{Sender, channel};

/// Mirrors the binary-private confirmation statement; the pause behavior,
/// not the statement, is under test here.
const CONFIRMATION_STATEMENT: &str =
    "current_authority_within_original_bounds_and_no_unreconciled_withdrawal";

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

fn budget(total_secs: u64) -> (Budget, Instant) {
    let t0 = Instant::now();
    (
        Budget {
            remaining: secs(total_secs),
            last: t0,
            paused: false,
        },
        t0,
    )
}

#[test]
fn running_budget_charges_elapsed_time_and_expires() {
    let (mut budget, t0) = budget(30);
    assert!(!budget.wake(t0 + secs(10), None));
    assert_eq!(budget.remaining, secs(20));
    assert!(
        budget.wake(t0 + secs(30), None),
        "an exhausted budget expires at its bound"
    );
}

#[test]
fn pause_suspends_charge_and_resume_never_refills() {
    let (mut budget, t0) = budget(30);
    assert!(!budget.wake(t0 + secs(10), Some(Control::Pause)));
    assert!(budget.paused);
    assert_eq!(budget.remaining, secs(20));
    // A duplicate pause while suspended keeps the same frozen budget.
    assert!(!budget.wake(t0 + secs(25), Some(Control::Pause)));
    assert!(budget.paused);
    assert_eq!(budget.remaining, secs(20));
    // Resume restarts charging from the resume sample; time is never refilled.
    assert!(!budget.wake(t0 + secs(25), Some(Control::Resume)));
    assert!(!budget.paused);
    assert_eq!(budget.remaining, secs(20));
    assert!(!budget.wake(t0 + secs(35), None));
    assert_eq!(budget.remaining, secs(10));
}

#[test]
fn resume_while_running_charges_without_refill() {
    let (mut budget, t0) = budget(30);
    assert!(!budget.wake(t0 + secs(10), Some(Control::Resume)));
    assert!(!budget.paused);
    assert_eq!(budget.remaining, secs(20));
}

#[test]
fn late_pause_cannot_suspend_an_expired_budget() {
    let (mut budget, t0) = budget(30);
    assert!(
        budget.wake(t0 + secs(30), Some(Control::Pause)),
        "expiry is checked before applying the queued pause"
    );
    assert!(!budget.paused, "an expired budget is never suspended");
}

#[test]
fn expiry_precedes_a_queued_done_control() {
    let (mut budget, t0) = budget(30);
    assert!(
        budget.wake(t0 + secs(31), Some(Control::Done)),
        "a late completion cannot outrun an expired budget"
    );
}

#[test]
fn socket_policy_overrides_missing_zero_and_excessive_connect_timeouts() {
    for dsn in [
        "host=127.0.0.1 user=u password=p",
        "host=127.0.0.1 user=u password=p connect_timeout=0",
        "host=127.0.0.1 user=u password=p connect_timeout=9999",
    ] {
        let mut cfg: postgres::Config = dsn.parse().unwrap();
        apply_wait_policy(&mut cfg);
        assert_eq!(cfg.get_connect_timeout(), Some(&SOCKET_CONNECT));
    }
}

#[test]
fn socket_policy_preserves_other_options_and_appends_fixed_waits() {
    let mut cfg: postgres::Config =
        "host=127.0.0.1 user=u password=p application_name=dw_probe options='-c geqo=off'"
            .parse()
            .unwrap();
    apply_wait_policy(&mut cfg);
    assert_eq!(cfg.get_application_name(), Some("dw_probe"));
    assert_eq!(cfg.get_user(), Some("u"));
    assert_eq!(
        cfg.get_options(),
        Some("-c geqo=off -c statement_timeout=3000 -c lock_timeout=750")
    );
    let mut cfg: postgres::Config = "host=127.0.0.1 user=u password=p".parse().unwrap();
    apply_wait_policy(&mut cfg);
    assert_eq!(
        cfg.get_options(),
        Some("-c statement_timeout=3000 -c lock_timeout=750")
    );
}

#[test]
fn effective_wait_values_must_match_the_fixed_policy() {
    assert!(waits_qualified("3s", "750ms"));
    assert!(waits_qualified("3000ms", "750ms"));
    assert!(!waits_qualified("0", "750ms"), "a disabled timeout refuses");
    assert!(!waits_qualified("garbage", "750ms"));
    assert!(
        !waits_qualified("3s", "0"),
        "a disabled lock timeout refuses"
    );
    assert!(!waits_qualified("1.5s", "750ms"));
    assert!(!waits_qualified("2.9s", "750ms"));
    assert!(!waits_qualified("3s", "1.5s"));
}

/// Serializes the tests that install process-global command controls so the
/// inactive-handle assertion is deterministic under parallel unit tests.
static CONTROLS_LOCK: Mutex<()> = Mutex::new(());

fn request() -> PlanningRequest {
    PlanningRequest {
        engagement_id: EngagementId(uuid::Uuid::from_u128(0xf101)),
        campaign_id: CampaignId(uuid::Uuid::from_u128(0xf102)),
        purpose_ref: GoalRef(uuid::Uuid::from_u128(0xf103)),
        asset_ref: AssetRef(uuid::Uuid::from_u128(0xf104)),
        expected_mission_revision: 1,
        current_authority_confirmed: true,
    }
}

fn operation() -> OperationId {
    OperationId(uuid::Uuid::from_u128(0xf105))
}

/// Counts recovery lookups and mutations separately so a denied exchange is
/// provably side-effect free.
#[derive(Default)]
struct CountingStore {
    lookups: usize,
    mutations: usize,
}

impl PlanningStore for CountingStore {
    fn assess(
        &mut self,
        _request: &PlanningRequest,
        _operation: OperationId,
        recover: bool,
        _allocator: &mut dyn OperationAllocator,
    ) -> Res<Option<duskweave::planning::PlanningAssessed>> {
        if recover {
            self.lookups += 1;
        } else {
            self.mutations += 1;
        }
        Ok(None)
    }

    fn authority_withdrawn(&mut self, _request: &PlanningRequest) -> Res<bool> {
        Ok(false)
    }
}

struct FixedAllocator;

impl OperationAllocator for FixedAllocator {
    fn allocate(&mut self) -> Res<uuid::Uuid> {
        Ok(uuid::Uuid::from_u128(0xf106))
    }
}

/// Removes the installed controls even when an assertion fails, so parallel
/// unit tests never observe leaked guard state.
struct Installed(Sender<Control>);
impl Drop for Installed {
    fn drop(&mut self) {
        installed_controls().lock().unwrap().take();
    }
}

#[test]
fn response_pause_is_inactive_without_a_command_guard() {
    let _serial = CONTROLS_LOCK.lock().unwrap();
    assert!(installed_controls().lock().unwrap().is_none());
    drop(response_pause());
}

#[test]
fn response_pause_pauses_and_resumes_on_refusal() {
    let _serial = CONTROLS_LOCK.lock().unwrap();
    let (tx, rx) = channel();
    let _installed = Installed(tx);
    installed_controls()
        .lock()
        .unwrap()
        .replace(_installed.0.clone());
    let mut store = CountingStore::default();
    let mut out = Vec::new();
    let mut input: &[u8] = b"";
    let res = confirm_if_new(
        &mut FixedAllocator,
        &mut store,
        &request(),
        operation(),
        false,
        &mut input,
        &mut out,
    );
    assert_eq!(
        res.err(),
        Some(Fail::State("authority_confirmation_failed"))
    );
    assert_eq!(store.mutations, 0);
    assert!(
        String::from_utf8_lossy(&out).contains("confirm_current_authority"),
        "the pause wraps only the response read, not the challenge"
    );
    let controls: Vec<Control> = rx.try_iter().collect();
    assert!(matches!(
        controls.as_slice(),
        [Control::Pause, Control::Resume]
    ));
}

#[test]
fn response_pause_pauses_and_resumes_on_success() {
    let _serial = CONTROLS_LOCK.lock().unwrap();
    let (tx, rx) = channel();
    let _installed = Installed(tx);
    installed_controls()
        .lock()
        .unwrap()
        .replace(_installed.0.clone());
    let response = format!(
        "{{\"challenge_id\":\"{}\",\"statement\":\"{CONFIRMATION_STATEMENT}\"}}\n",
        uuid::Uuid::from_u128(0xf106)
    );
    let mut store = CountingStore::default();
    let mut out = Vec::new();
    let mut input: &[u8] = response.as_bytes();
    let outcome = confirm_if_new(
        &mut FixedAllocator,
        &mut store,
        &request(),
        operation(),
        false,
        &mut input,
        &mut out,
    );
    assert_eq!(outcome.unwrap(), None);
    assert_eq!(store.mutations, 1);
    let controls: Vec<Control> = rx.try_iter().collect();
    assert!(matches!(
        controls.as_slice(),
        [Control::Pause, Control::Resume]
    ));
}

#[test]
fn response_pause_survives_disconnected_controls() {
    let _serial = CONTROLS_LOCK.lock().unwrap();
    let (tx, rx) = channel::<Control>();
    let _installed = Installed(tx);
    installed_controls()
        .lock()
        .unwrap()
        .replace(_installed.0.clone());
    drop(rx);
    drop(response_pause());
}
