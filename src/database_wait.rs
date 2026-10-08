//! Binary-private CLI wait bounds for the local PostgreSQL lane: fixed
//! socket, startup, server and active-invocation limits under independent
//! watchdogs (packet DW-FIX-DATABASE-WAIT-BOUNDS policy). A watchdog stop is
//! an incomplete invocation with an unresolved outcome (exit status 124): it
//! never proves a database rollback, current authority or authoritative
//! absence, so callers reconcile the original operation on a fresh
//! connection before any retry. These are initial engineering limits for
//! this CLI under normal OS scheduling, not measured latency guarantees and
//! not a cancellation implementation for other clients, services or workers.

use duskweave::postgres_mission::qualify_runtime;
use duskweave::{Fail, Res};
use postgres::{Client, NoTls};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const SOCKET_CONNECT: Duration = Duration::from_secs(2);
const STARTUP: Duration = Duration::from_secs(5);
const COMMAND: Duration = Duration::from_secs(30);
const STATEMENT_MS: u64 = 3000;
const LOCK_MS: u64 = 750;
const WATCHDOG_EXIT: i32 = 124;
const UNQUALIFIED_WAITS: Fail = Fail::Config("unqualified_database_waits");
const GUARD_FAILED: Fail = Fail::Config("database_wait_guard_failed");

#[derive(Clone, Copy)]
enum Control {
    Pause,
    Resume,
    Done,
}

/// Cumulative remaining budget, sampled at `last`; `paused` freezes charging.
/// The clock starts in the caller before the monitor thread is spawned.
struct Budget {
    remaining: Duration,
    last: Instant,
    paused: bool,
}

impl Budget {
    /// The clock starts in the caller before the monitor thread is spawned.
    fn start(total: Duration) -> Self {
        Self {
            remaining: total,
            last: Instant::now(),
            paused: false,
        }
    }

    /// One pre-wait sample: charge running time since the last sample and
    /// report whether the budget is already exhausted. The monitor runs
    /// this before every channel wait — a delayed first entry included —
    /// so a wait only ever receives the charged remainder, and an expired
    /// budget exits without waiting again. Paused time is not charged.
    fn charge(&mut self, now: Instant) -> bool {
        if !self.paused {
            self.remaining = self.remaining.saturating_sub(now - self.last);
        }
        self.last = now;
        self.remaining.is_zero()
    }

    /// One wake with a single sample: charge running time, refuse an expired
    /// budget before applying a queued control, then apply the control. A
    /// late pause can never suspend an expired budget and a resume never
    /// refills time. Returns true when the budget is exhausted.
    fn wake(&mut self, now: Instant, control: Option<Control>) -> bool {
        if self.charge(now) {
            return true;
        }
        match control {
            Some(Control::Pause) => self.paused = true,
            Some(Control::Resume) => self.paused = false,
            _ => {}
        }
        false
    }
}

/// One control wait under the budget's charging state: a paused budget
/// waits indefinitely; a running budget waits at most its remaining time.
/// `Err(())` means the budget elapsed while running; `Ok(None)` means the
/// controls disconnected and the scope is disarmed.
fn next_control(
    paused: bool,
    remaining: Duration,
    controls: &Receiver<Control>,
) -> Result<Option<Control>, ()> {
    if paused {
        return Ok(controls.recv().ok());
    }
    match controls.recv_timeout(remaining) {
        Ok(control) => Ok(Some(control)),
        Err(RecvTimeoutError::Timeout) => Err(()),
        Err(RecvTimeoutError::Disconnected) => Ok(None),
    }
}

/// Watchdog loop: charges running time before every channel wait — the
/// delayed first entry included — so a wait receives only the charged
/// remainder and an already-exhausted budget exits immediately. Std
/// waiting only; expiry terminates the process without taking
/// stdout/stderr locks and without waiting for driver or transaction
/// destructors.
fn monitor(mut budget: Budget, controls: Receiver<Control>) {
    while !budget.charge(Instant::now()) {
        match next_control(budget.paused, budget.remaining, &controls) {
            Err(()) => break,
            Ok(control) => {
                if budget.wake(Instant::now(), control) {
                    break;
                }
                if matches!(control, None | Some(Control::Done)) {
                    return;
                }
            }
        }
    }
    std::process::exit(WATCHDOG_EXIT);
}

fn spawn_monitor(budget: Budget) -> Res<Sender<Control>> {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("database_wait_watchdog".into())
        .spawn(move || monitor(budget, rx))
        .map_err(|_| GUARD_FAILED)?;
    Ok(tx)
}

static COMMAND_CONTROLS: OnceLock<Mutex<Option<Sender<Control>>>> = OnceLock::new();

fn installed_controls() -> &'static Mutex<Option<Sender<Control>>> {
    COMMAND_CONTROLS.get_or_init(|| Mutex::new(None))
}

/// Cumulative 30-second guard around one CLI command dispatch. Setup failure
/// refuses; there is no unguarded fallback. Normal return or unwind disarms.
pub(super) struct CommandGuard;

impl CommandGuard {
    pub(super) fn start() -> Res<Self> {
        let tx = spawn_monitor(Budget::start(COMMAND))?;
        installed_controls()
            .lock()
            .map_err(|_| GUARD_FAILED)?
            .replace(tx);
        Ok(Self)
    }
}

impl Drop for CommandGuard {
    fn drop(&mut self) {
        if let Ok(mut installed) = installed_controls().lock()
            && let Some(tx) = installed.take()
        {
            let _ = tx.send(Control::Done);
        }
    }
}

/// Scope guard around the live C0 response read: pauses the command budget on
/// entry and resumes on every success, error or panic return. Without an
/// installed command guard the handle is inactive.
pub(super) struct ResponsePause {
    controls: Option<Sender<Control>>,
}

pub(super) fn response_pause() -> ResponsePause {
    let controls = installed_controls()
        .lock()
        .ok()
        .and_then(|installed| installed.as_ref().cloned());
    if let Some(controls) = &controls {
        let _ = controls.send(Control::Pause);
    }
    ResponsePause { controls }
}

impl Drop for ResponsePause {
    fn drop(&mut self) {
        if let Some(controls) = self.controls.take() {
            let _ = controls.send(Control::Resume);
        }
    }
}

/// Fixed wait policy for every CLI connection: a 2-second socket-level
/// connect limit overriding any DSN value, and fixed server settings appended
/// after existing options with unambiguous separation. Other configuration,
/// including options used to reject weakened durability, is preserved.
fn apply_wait_policy(cfg: &mut postgres::Config) {
    cfg.connect_timeout(SOCKET_CONNECT);
    let existing = cfg.get_options().unwrap_or_default();
    let waits = format!("{existing} -c statement_timeout={STATEMENT_MS} -c lock_timeout={LOCK_MS}");
    cfg.options(waits.trim_start());
}

/// Millisecond factor of a pg_settings display unit; an unrecognized unit
/// makes the value unverifiable.
fn unit_ms(unit: &str) -> Option<f64> {
    Some(match unit {
        "" | "ms" => 1.0,
        "s" => 1_000.0,
        "min" => 60_000.0,
        "h" => 3_600_000.0,
        "d" => 86_400_000.0,
        _ => return None,
    })
}

/// Server display form to milliseconds, for example "3s", "750ms", "1.5s", "0".
fn wait_ms(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    let digits = raw
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(raw.len());
    let (num, unit) = raw.split_at(digits);
    Some((num.parse::<f64>().ok()? * unit_ms(unit)?).round() as u64)
}

/// Both effective session values must equal the fixed policy before business
/// use; mismatch or read failure refuses without value-bearing diagnostics.
fn waits_qualified(statement: &str, lock: &str) -> bool {
    wait_ms(statement) == Some(STATEMENT_MS) && wait_ms(lock) == Some(LOCK_MS)
}

fn verify_effective_waits(client: &mut Client) -> Res<()> {
    let row = client
        .query_one(
            "SELECT (SELECT setting FROM pg_settings WHERE name = 'statement_timeout'), \
             (SELECT setting FROM pg_settings WHERE name = 'lock_timeout')",
            &[],
        )
        .map_err(|_| UNQUALIFIED_WAITS)?;
    if waits_qualified(row.get(0), row.get(1)) {
        Ok(())
    } else {
        Err(UNQUALIFIED_WAITS)
    }
}

/// Every CLI connection: the fixed wait policy, effective-value verification
/// and the unchanged role/durability qualification, all inside one
/// independent 5-second startup guard spanning connect, authentication and
/// qualification.
pub(super) fn connect_qualified(mut cfg: postgres::Config) -> Res<Client> {
    let done = spawn_monitor(Budget::start(STARTUP))?;
    let outcome = (|| {
        apply_wait_policy(&mut cfg);
        let mut client = cfg
            .connect(NoTls)
            .map_err(|_| Fail::Config("connect_failed"))?;
        verify_effective_waits(&mut client)?;
        qualify_runtime(&mut client)?;
        Ok(client)
    })();
    let _ = done.send(Control::Done);
    outcome
}

#[cfg(test)]
#[path = "../tests/faults/database_wait_faults.rs"]
mod fault_tests;
