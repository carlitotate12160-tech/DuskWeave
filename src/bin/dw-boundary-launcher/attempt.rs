//! Root-owned attempt state and the shared monotonic grant/STOP guard.
pub(super) mod https;
use duskweave::m1_dns::DnsRequest;
use duskweave::{Fail, Res};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use uuid::Uuid;
const ROOT: &str = "/run/duskweave/launcher";
#[cfg(test)]
thread_local! {
    static TEST_ROOT: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
pub(super) fn set_test_root(path: Option<PathBuf>) {
    TEST_ROOT.with(|c| *c.borrow_mut() = path);
}
fn root() -> PathBuf {
    #[cfg(test)]
    if let Some(r) = TEST_ROOT.with(|c| c.borrow().clone()) {
        return r;
    }
    PathBuf::from(ROOT)
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Session {
    #[serde(skip)]
    pub preparation_deadline: Option<Instant>,
    pub run_id: Uuid,
    pub request: Box<DnsRequest>,
    pub opened_at_ms: i64,
    pub ready_at_ms: Option<i64>,
    pub attempt_deadline_at_ms: Option<i64>,
    pub handling_deadline_at_ms: i64,
}
pub(super) fn dir(attempt: Uuid) -> PathBuf {
    root().join(attempt.simple().to_string())
}
fn stop_path(e: Uuid, c: Uuid, g: u64) -> PathBuf {
    root().join(format!("stop-{}-{}-{g}", e.simple(), c.simple()))
}
pub(super) struct Guard(File);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = rustix::fs::flock(&self.0, rustix::fs::FlockOperation::Unlock);
    }
}
fn guard(e: Uuid, c: Uuid, end: Instant) -> Res<Guard> {
    guard_path(format!("campaign-guard-{}-{}", e.simple(), c.simple()), end)
}
fn run_guard(run: Uuid, end: Instant) -> Res<Guard> {
    guard_path(format!("run-guard-{}", run.simple()), end)
}
pub(super) fn session_guard_ids(run: Uuid, e: Uuid, c: Uuid, end: Instant) -> Res<(Guard, Guard)> {
    Ok((run_guard(run, end)?, guard(e, c, end)?))
}
fn guard_path(name: String, end: Instant) -> Res<Guard> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(root().join(name))
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    loop {
        match rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => break,
            Err(rustix::io::Errno::WOULDBLOCK) if Instant::now() < end => {
                std::thread::sleep(Duration::from_millis(1))
            }
            _ => return Err(Fail::State("live_host_guard_unavailable")),
        }
    }
    Ok(Guard(file))
}
fn campaign_path(e: Uuid, c: Uuid) -> PathBuf {
    root().join(format!("campaign-stop-{}-{}", e.simple(), c.simple()))
}
fn run_path(run: Uuid) -> PathBuf {
    root().join(format!("run-stop-{}", run.simple()))
}
pub(super) fn stopped_ids(e: Uuid, c: Uuid, g: u64) -> bool {
    stop_path(e, c, g).exists() || campaign_path(e, c).exists()
}
pub(super) fn blocked_ids(e: Uuid, c: Uuid, g: u64, run: Uuid) -> bool {
    stopped_ids(e, c, g) || run_path(run).exists()
}
pub(super) fn stopped(q: &DnsRequest) -> bool {
    stopped_ids(q.engagement_id, q.campaign_id, q.session_generation)
}
fn blocked(s: &Session) -> bool {
    blocked_ids(
        s.request.engagement_id,
        s.request.campaign_id,
        s.request.session_generation,
        s.run_id,
    )
}
fn session_guard(s: &Session, end: Instant) -> Res<(Guard, Guard)> {
    session_guard_ids(
        s.run_id,
        s.request.engagement_id,
        s.request.campaign_id,
        end,
    )
}
pub(super) fn owned_root() -> Res<()> {
    let m = std::fs::symlink_metadata(root()).map_err(|_| Fail::Config("live_host_unqualified"))?;
    let f = super::custody::observed(&m);
    super::custody::control_root(&f)
        .then_some(())
        .ok_or(Fail::Config("live_host_unqualified"))
}
pub(super) fn load(attempt: Uuid, generation: u64) -> Res<Session> {
    let session = record(attempt)?;
    if session.request.session_generation != generation {
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(session)
}
pub(super) fn qualify_session_meta(m: &std::fs::Metadata) -> bool {
    let f = super::custody::observed(m);
    super::custody::session_journal(&f)
}

fn record(attempt: Uuid) -> Res<Session> {
    let path = dir(attempt).join("session.json");
    let meta =
        std::fs::symlink_metadata(&path).map_err(|_| Fail::Config("live_host_unqualified"))?;
    if !qualify_session_meta(&meta) {
        return Err(Fail::Config("live_host_unqualified"));
    }
    let file = File::open(path).map_err(|_| Fail::Config("live_host_unqualified"))?;
    let session: Session = serde_json::from_reader(file.take(16385))
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    if session.request.attempt != attempt {
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(session)
}
fn save(session: &Session) -> Res<()> {
    save_json(session.request.attempt, session)
}
pub(super) fn save_json<T: Serialize>(attempt: Uuid, session: &T) -> Res<()> {
    let path = dir(attempt).join("session.next");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    let bytes = serde_json::to_vec(session).map_err(|_| Fail::Config("live_host_unqualified"))?;
    file.write_all(&bytes)
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    drop(file);
    std::fs::rename(path, dir(attempt).join("session.json"))
        .map_err(|_| Fail::Config("live_host_unqualified"))
}
pub(super) fn open(run_id: Uuid, request: &DnsRequest) -> Res<Session> {
    let monotonic = Instant::now();
    let now = validate_open(run_id, request)?;
    let session = new_session(run_id, request, now, monotonic);
    let _guard = session_guard(&session, monotonic + Duration::from_millis(1000))?;
    if blocked(&session) {
        return Err(Fail::State("live_effect_stopped"));
    }
    create_dir(session.request.attempt)?;
    save(&session)?;
    Ok(session)
}
pub(super) fn create_dir(attempt: Uuid) -> Res<()> {
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(dir(attempt))
        .map_err(|_| Fail::Config("live_attempt_already_exists"))
}
fn validate_open(run: Uuid, request: &DnsRequest) -> Res<i64> {
    owned_root()?;
    let now = super::qualification::now_ms()?;
    request.validate(now)?;
    if run.is_nil() {
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(now)
}
fn new_session(run_id: Uuid, request: &DnsRequest, now: i64, monotonic: Instant) -> Session {
    let prep_ms = (request.deadline_at_ms - now).min(5000) as u64;
    Session {
        preparation_deadline: Some(monotonic + Duration::from_millis(prep_ms)),
        run_id,
        request: Box::new(request.clone()),
        opened_at_ms: now,
        ready_at_ms: None,
        attempt_deadline_at_ms: None,
        handling_deadline_at_ms: request.deadline_at_ms.min(now + 5000),
    }
}
pub(super) fn ready(attempt: Uuid, generation: u64) -> Res<()> {
    let mut s = load(attempt, generation)?;
    let _guard = session_guard(&s, Instant::now() + Duration::from_millis(1000))?;
    let now = super::qualification::now_ms()?;
    if blocked(&s) || now >= s.handling_deadline_at_ms || s.ready_at_ms.is_some() {
        return Err(Fail::State("live_effect_stopped"));
    }
    s.ready_at_ms = Some(now);
    s.handling_deadline_at_ms = s.request.deadline_at_ms.min(now + 1000);
    save(&s)
}
pub(super) fn start(
    attempt: Uuid,
    generation: u64,
    deadline: i64,
    release: impl FnOnce() -> Res<()>,
) -> Res<()> {
    let mut s = load(attempt, generation)?;
    let _guard = session_guard(&s, Instant::now() + Duration::from_millis(1000))?;
    let now = super::qualification::now_ms()?;
    validate_start(&s, now, deadline)?;
    charge(&mut s, deadline)?;
    super::network::grant(attempt, deadline)?;
    release().inspect_err(|_| {
        let _ = super::network::revoke(attempt);
    })
}
fn charge(s: &mut Session, deadline: i64) -> Res<()> {
    let _one_use = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(dir(s.request.attempt).join("started"))
        .map_err(|_| Fail::State("live_effect_already_charged"))?;
    s.attempt_deadline_at_ms = Some(deadline);
    s.handling_deadline_at_ms = deadline.saturating_add(1000);
    save(s)
}
fn validate_start(s: &Session, now: i64, deadline: i64) -> Res<()> {
    let valid = !blocked(s)
        && s.ready_at_ms.is_some()
        && now < s.handling_deadline_at_ms
        && deadline > now
        && deadline <= s.request.deadline_at_ms
        && deadline <= now + 2000
        && s.attempt_deadline_at_ms.is_none();
    valid
        .then_some(())
        .ok_or(Fail::State("live_effect_start_refused"))
}
pub(super) fn renew(attempt: Uuid, generation: u64, deadline: i64) -> Res<()> {
    let s = load(attempt, generation)?;
    let _guard = session_guard(&s, Instant::now() + Duration::from_millis(1000))?;
    let original = s
        .attempt_deadline_at_ms
        .ok_or(Fail::State("live_effect_start_refused"))?;
    if blocked(&s) || deadline > original {
        return Err(Fail::State("live_effect_stopped"));
    }
    super::network::grant(attempt, deadline)
}
fn latch(path: PathBuf) -> Res<()> {
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(_) => Err(Fail::Config("live_host_unqualified")),
    }
}
#[derive(Clone, Copy)]
enum StopScope {
    Generation(Uuid, Uuid, u64),
    Campaign(Uuid, Uuid),
    Run(Uuid),
}
impl StopScope {
    fn path(self) -> Res<PathBuf> {
        let (ids, generation, path) = match self {
            Self::Run(run) => ([run, run], 1, run_path(run)),
            Self::Campaign(e, c) => ([e, c], 1, campaign_path(e, c)),
            Self::Generation(e, c, g) => ([e, c], g, stop_path(e, c, g)),
        };
        if ids.into_iter().any(|id| id.is_nil()) || generation == 0 {
            return Err(Fail::Input("invalid_input"));
        }
        Ok(path)
    }
    fn latch(self, end: Instant) -> Res<()> {
        owned_root()?;
        let path = self.path()?;
        let _guard = match self {
            Self::Run(run) => run_guard(run, end)?,
            Self::Campaign(e, c) | Self::Generation(e, c, _) => guard(e, c, end)?,
        };
        latch(path)
    }
    fn matches_ids(self, run: Uuid, e: Uuid, c: Uuid, generation: u64) -> bool {
        match self {
            Self::Generation(te, tc, tg) => (te, tc, tg) == (e, c, generation),
            Self::Campaign(te, tc) => (te, tc) == (e, c),
            Self::Run(trun) => trun == run,
        }
    }
    #[cfg(test)]
    fn matches(self, s: &Session) -> bool {
        self.matches_ids(
            s.run_id,
            s.request.engagement_id,
            s.request.campaign_id,
            s.request.session_generation,
        )
    }
}
fn stop_ids(attempt: Uuid) -> Option<(Uuid, Uuid, Uuid, u64)> {
    record(attempt)
        .ok()
        .map(|s| {
            (
                s.run_id,
                s.request.engagement_id,
                s.request.campaign_id,
                s.request.session_generation,
            )
        })
        .or_else(|| https::stop_ids(attempt))
}
pub(super) fn stop(e: Uuid, c: Uuid, g: u64) -> bool {
    stop_until(e, c, g, Instant::now() + Duration::from_millis(1000))
}
pub(super) fn stop_until(e: Uuid, c: Uuid, g: u64, end: Instant) -> bool {
    stop_scope(StopScope::Generation(e, c, g), end).1
}
pub(super) fn stop_run(run: Uuid) -> (bool, bool) {
    let deadline = Instant::now() + Duration::from_millis(1000);
    stop_scope(StopScope::Run(run), deadline)
}
pub(super) fn stop_campaign(e: Uuid, c: Uuid) -> (bool, bool) {
    let deadline = Instant::now() + Duration::from_millis(1000);
    stop_scope(StopScope::Campaign(e, c), deadline)
}
fn stop_scope(scope: StopScope, end: Instant) -> (bool, bool) {
    if scope.latch(end).is_err() {
        return (false, false);
    }
    let Ok(entries) = std::fs::read_dir(root()) else {
        return (true, false);
    };
    let mut disposed = true;
    for entry in entries {
        disposed &= stop_entry(entry, scope, end);
    }
    (true, disposed && Instant::now() < end)
}
fn stop_entry(entry: std::io::Result<std::fs::DirEntry>, scope: StopScope, end: Instant) -> bool {
    let Ok(entry) = entry else {
        return false;
    };
    let Ok(attempt) = Uuid::parse_str(&entry.file_name().to_string_lossy()) else {
        return true;
    };
    let Some((run, e, c, g)) = stop_ids(attempt) else {
        return false;
    };
    if !scope.matches_ids(run, e, c, g) {
        return true;
    }
    // Wait out any in-flight grant under the same guard, then reap outside it.
    let Ok(guard) = guard(e, c, end) else {
        return false;
    };
    drop(guard);
    dispose_until(attempt, end)
}
pub(super) fn dispose(attempt: Uuid) -> bool {
    dispose_until(attempt, Instant::now() + Duration::from_millis(1000))
}
pub(super) fn dispose_until(attempt: Uuid, end: Instant) -> bool {
    let _ = super::network::revoke_until(
        attempt,
        end.min(Instant::now() + Duration::from_millis(200)),
    );
    let containers = super::docker::remove(attempt, false, end).is_ok()
        & super::docker::remove(attempt, true, end).is_ok();
    let network = super::network::destroy(attempt, end);
    let pending = dir(attempt).join("transport.pending").exists();
    if !containers || !network || pending || Instant::now() >= end {
        return false;
    }
    std::fs::remove_dir_all(dir(attempt)).is_ok() || !dir(attempt).exists()
}
#[cfg(test)]
#[path = "../../../tests/m1_live_dns/units/launcher_attempt.rs"]
pub(crate) mod tests;
