//! Same-host revocation only. UUID-derived write-once markers never confer authority.
use super::control_domain::m1_reasoning::transport_control::Permit;
use super::control_domain::m1_run::RunRequest;
use super::control_domain::{Fail, Res};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};
use uuid::Uuid;

const UNAVAILABLE: Fail = Fail::Config("run_control_unqualified");

/// Closed launch hint: the parent marks provider decision children so the child
/// can arm preparation protection before its first database wait. It grants no
/// authority and is verified against the authoritatively claimed run/phase.
#[allow(dead_code)]
pub(super) const PROVIDER_HINT: &str = "DW_M1_RUN_PROVIDER_DECISION";
/// Bounded readiness frame prefix on the fixed child's stdout pipe.
#[allow(dead_code)]
pub(super) const READY_PREFIX: &str = "m1_run_ready_v1";
/// The single readiness frame binding the preallocated run UUID and exact phase.
#[allow(dead_code)]
pub(super) fn ready_frame(run: Uuid, phase: &str) -> String {
    format!("{READY_PREFIX} {run} {phase}")
}
/// Fixed 5000ms launch→readiness policy bound. The owner environment may
/// override it only for instrumented qualification runs where the same
/// protocol must hold at a slower execution speed; invalid values keep the
/// policy bound.
#[allow(dead_code)]
pub(super) fn preparation_bound() -> Duration {
    std::env::var_os("DW_M1_RUN_PREPARATION_MS")
        .and_then(|v| v.to_str()?.parse::<u64>().ok())
        .filter(|ms| (100..=120000).contains(ms))
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(5000))
}
#[derive(Clone)]
pub(super) struct Binding {
    root: PathBuf,
    run: Uuid,
    engagement: Uuid,
    campaign: Uuid,
}
fn safe_root(root: &Path) -> Res<()> {
    if !local_absolute(root) || root.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(UNAVAILABLE);
    }
    for ancestor in root.ancestors() {
        let meta = fs::symlink_metadata(ancestor).map_err(|_| UNAVAILABLE)?;
        if !meta.is_dir() || redirected(&meta) {
            return Err(UNAVAILABLE);
        }
    }
    Ok(())
}
#[cfg(windows)]
fn local_absolute(root: &Path) -> bool {
    use std::path::Prefix;
    root.is_absolute()
        && matches!(root.components().next(), Some(Component::Prefix(p))
        if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
}
#[cfg(not(windows))]
fn local_absolute(root: &Path) -> bool {
    root.is_absolute()
}
#[cfg(windows)]
fn redirected(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn redirected(meta: &fs::Metadata) -> bool {
    meta.file_type().is_symlink()
}
#[cfg(unix)]
fn private_access(root: &Path) -> Res<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let metadata = fs::metadata(root).map_err(|_| UNAVAILABLE)?;
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(UNAVAILABLE);
    }
    let probe = write_probe(root)?;
    let owned = fs::metadata(&probe).map_err(|_| UNAVAILABLE)?.uid() == metadata.uid();
    fs::remove_file(probe).map_err(|_| UNAVAILABLE)?;
    owned.then_some(()).ok_or(UNAVAILABLE)
}
fn write_probe(root: &Path) -> Res<PathBuf> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| UNAVAILABLE)?
        .as_nanos();
    let probe = root.join(format!("qualification-{}-{stamp}", std::process::id()));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|_| UNAVAILABLE)?;
    Ok(probe)
}
#[cfg(windows)]
fn private_access(root: &Path) -> Res<()> {
    use std::process::{Command, Stdio};
    // Fixed read-only ACL query. No input is interpolated into script text.
    let script = "$ErrorActionPreference='Stop'; $a=Get-Acl -LiteralPath $env:DW_M1_RUN_CONTROL_ROOT; \
        $s=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value; \
        if($a.GetOwner([Security.Principal.SecurityIdentifier]).Value -ne $s){exit 1}; \
        foreach($r in $a.Access){if($r.AccessControlType -eq 'Allow' -and \
        $r.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value -notin \
        @($s,'S-1-5-18','S-1-5-32-544')){exit 1}}; exit 0";
    let system = std::env::var_os("SystemRoot").ok_or(UNAVAILABLE)?;
    let child =
        Command::new(Path::new(&system).join("System32/WindowsPowerShell/v1.0/powershell.exe"))
            .env_clear()
            .env("SystemRoot", system)
            .env("DW_M1_RUN_CONTROL_ROOT", root)
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| UNAVAILABLE)?;
    wait_acl_probe(child)?;
    let probe = write_probe(root)?;
    fs::remove_file(probe).map_err(|_| UNAVAILABLE)
}
#[cfg(windows)]
fn wait_acl_probe(mut child: std::process::Child) -> Res<()> {
    let end = Instant::now() + Duration::from_millis(1500);
    loop {
        if let Some(status) = child.try_wait().map_err(|_| UNAVAILABLE)? {
            return status.success().then_some(()).ok_or(UNAVAILABLE);
        }
        if Instant::now() >= end {
            let _ = child.kill();
            let _ = child.wait();
            return Err(UNAVAILABLE);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn root() -> Res<PathBuf> {
    let root = PathBuf::from(std::env::var_os("DW_M1_RUN_CONTROL_ROOT").ok_or(UNAVAILABLE)?);
    safe_root(&root)?;
    private_access(&root)?;
    Ok(root)
}
fn run_marker(root: &Path, run: Uuid) -> PathBuf {
    root.join(format!("{run}.run-stop"))
}
fn campaign_marker(root: &Path, e: Uuid, c: Uuid) -> PathBuf {
    root.join(format!("{e}.{c}.campaign-stop"))
}
fn absent(path: &Path) -> Res<bool> {
    match fs::symlink_metadata(path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Ok(_) => Ok(false),
        Err(_) => Err(UNAVAILABLE),
    }
}
fn write_stop(path: &Path) -> Res<()> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => file.write_all(b"STOP\n").map_err(|_| UNAVAILABLE),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(_) => Err(UNAVAILABLE),
    }
}
impl Binding {
    pub(super) fn qualify(q: &RunRequest) -> Res<Self> {
        q.validate()?;
        let binding = Self {
            root: root()?,
            run: q.run_id,
            engagement: q.context.mission.engagement_id.0,
            campaign: q.context.mission.campaign_id.0,
        };
        binding.current()?;
        Ok(binding)
    }
    pub(super) fn current(&self) -> Res<()> {
        safe_root(&self.root)?;
        let clear = absent(&run_marker(&self.root, self.run))?
            && absent(&campaign_marker(&self.root, self.engagement, self.campaign))?;
        clear
            .then_some(())
            .ok_or(Fail::State("run_control_stopped"))
    }
}

/// Bound caller waiting even if local storage stalls. Failure never skips the SQL stop attempt.
fn bounded_signal(work: impl FnOnce() -> Res<()> + Send + 'static) -> Res<()> {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("run_local_stop".into())
        .spawn(move || {
            let _ = tx.send(work());
        })
        .map_err(|_| UNAVAILABLE)?;
    rx.recv_timeout(Duration::from_millis(2000))
        .map_err(|_| UNAVAILABLE)?
}
pub(super) fn signal_run(run: Uuid) -> Res<()> {
    if std::env::var_os("DW_M1_RUN_CONTROL_ROOT").is_none() {
        return Ok(());
    }
    bounded_signal(move || write_stop(&run_marker(&root()?, run)))
}
pub(super) fn signal_campaign(e: Uuid, c: Uuid) -> Res<()> {
    if std::env::var_os("DW_M1_RUN_CONTROL_ROOT").is_none() {
        return Ok(());
    }
    bounded_signal(move || write_stop(&campaign_marker(&root()?, e, c)))
}

/// The parent owns this pipe; unexpected data and EOF can only revoke.
pub(super) fn read_parent(permit: Permit) -> Res<()> {
    std::thread::Builder::new()
        .name("run_parent_control".into())
        .spawn(move || {
            let mut byte = [0u8; 1];
            let _ = std::io::stdin().read(&mut byte);
            permit.stop();
        })
        .map_err(|_| UNAVAILABLE)?;
    Ok(())
}

pub(super) struct Preparation(Sender<()>);
impl Preparation {
    pub(super) fn start() -> Res<Self> {
        let (tx, rx) = channel();
        let end = Instant::now() + preparation_bound();
        std::thread::Builder::new()
            .name("run_inference_preparation".into())
            .spawn(move || {
                if matches!(
                    rx.recv_timeout(end.saturating_duration_since(Instant::now())),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                ) {
                    std::process::exit(124);
                }
            })
            .map_err(|_| UNAVAILABLE)?;
        Ok(Self(tx))
    }
}
impl Drop for Preparation {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}
