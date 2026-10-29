#![allow(dead_code)]
use crate::{db_support, native, terrain};
use serde_json::{Value, json};
use std::process::Command;
use uuid::Uuid;
pub fn refresh() {
    refresh_legacy();
    native::admin()
        .batch_execute(include_str!(
            "../../migrations/0021_inference_lifecycle.sql"
        ))
        .unwrap();
}
pub fn refresh_legacy() {
    let mut a = native::admin();
    a.batch_execute(
        &include_str!("../../migrations/0016_autonomous_inference_profile.sql")
            .replace("CREATE FUNCTION ", "CREATE OR REPLACE FUNCTION "),
    )
    .unwrap();
    for source in [
        include_str!("../../migrations/0017_autonomous_fixture_run.sql"),
        include_str!("../../migrations/0018_issued_fixture_terrain.sql"),
        include_str!("../../migrations/0019_provider_fixture_feedback.sql"),
        include_str!("../../migrations/0020_m1_run_diagnostics.sql"),
    ] {
        let normalized = source.replace("CREATE OR REPLACE FUNCTION ", "CREATE FUNCTION ");
        for function in normalized.split("CREATE FUNCTION ").skip(1) {
            let end = function.find("$$;").unwrap() + 3;
            a.batch_execute(&format!("CREATE OR REPLACE FUNCTION {}", &function[..end]))
                .unwrap();
        }
    }
}
pub fn registered(slot: u128) -> Value {
    registered_with(slot, |_| {})
}
pub fn registered_with(slot: u128, change: impl FnOnce(&mut Value)) -> Value {
    let (e, c) = db_support::scope(slot);
    let now: i64 = native::admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    let mut raw = terrain::combined_registration_json(e, c, now, true);
    let p = &mut raw["m1_inference_permission"];
    p["version"] = json!("m1_inference_permission_v3");
    p["recipient"] = json!("urn:duskweave:fixture:inference:v1");
    p["profile"] = json!("deterministic-autonomous-v1");
    p["execution_mode"] = json!("controlled_fixture");
    p["run_profile"] = json!("autonomous-dns-fixture-v1");
    p["field_classes"] = json!([
        "mission_question_constraints",
        "scoped_opaque_references",
        "owner_qualified_summaries",
        "admitted_operator_premises",
        "manifest_availability",
        "omissions_unknown",
        "capacity",
        "admitted_repair_fields_codes",
        "admitted_campaign_decision_feedback"
    ]);
    p["campaign_limits"] = json!({"calls":4,"request_bytes":65536,"response_bytes":262144,"output_token_capacity":16384,"wall_time_ms":90000});
    for (k, v) in [
        ("context_bytes", 8192),
        ("decision_wall_time_ms", 45000),
        ("per_call_wall_time_ms", 20000),
        ("shared_wall_time_ms", 40000),
        ("phase_wall_time_ms", 50000),
        ("cancel_grace_ms", 2000),
        ("accounting_window_ms", 3000),
    ] {
        p["operation_limits"][k] = json!(v);
    }
    raw["included_assets"] = json!([Uuid::from_u128(0x21)]);
    raw["m1_permission"]["contact_rules"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    change(&mut raw);
    let input = duskweave::input::parse_register(&serde_json::to_vec(&raw).unwrap()).unwrap();
    let (mut a, mut m, mut h) = db_support::ports();
    let op = duskweave::registration::prepare_operation(&mut a).unwrap();
    duskweave::registration::register(&mut a, &mut m, &mut h, op, &input).unwrap();
    let candidates = raw["included_assets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                Uuid::parse_str(v.as_str().unwrap()).unwrap(),
                duskweave::m1_context_input::Priority::High,
            )
        })
        .collect();
    let context = terrain::context_request_for(e, c, &raw, candidates).0;
    let seeds:Vec<_>=raw["m1_permission"]["contact_rules"].as_array().unwrap().iter().map(|r|
        json!({"asset_ref":r["asset_ref"],"name":r["rule"].get("exact").or_else(||r["rule"].get("label_suffix")).unwrap()})).collect();
    json!({"version":"m1_autonomous_run_request_v1","run_id":consent_id(),"run_profile":raw["m1_inference_permission"]["run_profile"],
        "execution_mode":"controlled_fixture","deadline_at_ms":now*1000+149000,"context":context,
        "seed_names":seeds})
}
pub fn consent_id() -> Uuid {
    native::client("DW_TEST_DATABASE_URL")
        .query_one("SELECT gen_random_uuid()", &[])
        .unwrap()
        .get(0)
}
pub fn command() -> Command {
    let mut c = Command::new(actual_binary());
    c.env_clear();
    c.env("DW_DATABASE_CONFIG_MODE", "env-local").env(
        "DW_DATABASE_URL",
        std::env::var("DW_TEST_DATABASE_URL").unwrap(),
    );
    for k in [
        "DW_INFERENCE_BROKER_DATABASE_URL",
        "DW_COGNITION_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL",
        "DW_TERRAIN_DATABASE_URL",
        "SystemRoot",
        "LLVM_PROFILE_FILE",
    ] {
        if let Some(v) = std::env::var_os(k) {
            c.env(k, v);
        }
    }
    c
}
fn actual_binary() -> std::path::PathBuf {
    // Test custodian only: an isolated negative-control binary must retain
    // its actual executable identity. This is never a production child flag.
    std::env::var_os("DW_TEST_RUN_BINARY")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_duskweave").into())
}

/// Exact direct phase identity. Cleanup opens an OS handle before rechecking
/// birth identity, so a recycled PID can never authorize a custodian kill.
struct PhaseIdentity {
    pid: u32,
    birth: u64,
    started: std::time::Instant,
    run: Uuid,
}
fn observation_output(command: Command) -> Option<std::process::Output> {
    observation_output_until(
        command,
        std::time::Instant::now() + std::time::Duration::from_secs(3),
    )
}
fn observation_output_until(
    mut command: Command,
    end: std::time::Instant,
) -> Option<std::process::Output> {
    use std::process::Stdio;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    while !matches!(child.try_wait(), Ok(Some(_))) {
        if std::time::Instant::now() >= end {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    child.wait_with_output().ok().filter(|o| o.status.success())
}
#[cfg(windows)]
fn powershell(script: &str) -> Command {
    let mut c = Command::new(
        std::path::Path::new(&std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe"),
    );
    c.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    c
}
#[cfg(windows)]
fn phase_identity_script(parent: u32, run: Uuid) -> String {
    // Discovery can outlive a process. Hold the native handle and revalidate
    // parent/run ownership before accepting its birth identity for cleanup.
    format!(
        "$ErrorActionPreference='Stop'; $c=@(Get-CimInstance Win32_Process -Filter \"ParentProcessId={parent}\" | Where-Object {{ $_.CommandLine -like '*m1-run-phase*{run}*' }}); if ($c.Count -eq 1) {{ try {{ $p=[Diagnostics.Process]::GetProcessById($c[0].ProcessId); $null=$p.Handle; if (!$p.HasExited) {{ $confirmed=@(Get-CimInstance Win32_Process -Filter \"ProcessId=$($p.Id) AND ParentProcessId={parent}\" | Where-Object {{ $_.CommandLine -like '*m1-run-phase*{run}*' }}); if ($confirmed.Count -eq 1 -and !$p.HasExited) {{ [Console]::WriteLine(('{{0}} {{1}}' -f $p.Id,($p.StartTime.ToUniversalTime().Ticks))) }} }} }} catch [ArgumentException] {{ }} }}"
    )
}
#[cfg(windows)]
fn phase_identity(parent: u32, run: Uuid) -> Option<PhaseIdentity> {
    let output = observation_output(powershell(&phase_identity_script(parent, run)))?;
    let text = std::str::from_utf8(&output.stdout).ok()?;
    let (pid, birth) = text.trim().split_once(' ')?;
    let birth = birth.parse().ok()?;
    Some(PhaseIdentity {
        pid: pid.parse().ok()?,
        birth,
        started: wall_instant((birth - 621355968000000000) / 10000),
        run,
    })
}
/// Replay a stale discovery row against a live, test-owned replacement PID.
/// Only the first CIM snapshot is substituted; handle lookup and subsequent
/// discovery use the real OS. The replacement is never killed by this probe.
#[cfg(windows)]
pub fn stale_phase_snapshot_accepted(parent: u32, run: Uuid, replacement: u32) -> bool {
    let snapshot = format!(
        "$script:discoveryCalls=0; function Get-CimInstance {{ param([string]$ClassName,[string]$Filter); $script:discoveryCalls++; if ($script:discoveryCalls -eq 1) {{ [pscustomobject]@{{ ProcessId={replacement}; ParentProcessId={parent}; CommandLine='m1-run-phase {run}'; CreationDate=[DateTime]::UtcNow.AddMinutes(-1) }} }} else {{ CimCmdlets\\Get-CimInstance -ClassName $ClassName -Filter $Filter }} }}; "
    );
    let script = snapshot + &phase_identity_script(parent, run);
    let output = observation_output(powershell(&script))
        .expect("stale snapshot observation must complete successfully");
    !output.stdout.is_empty()
}
pub fn wall_instant(ms: u64) -> std::time::Instant {
    let wall = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    std::time::Instant::now() - wall.saturating_sub(std::time::Duration::from_millis(ms))
}
#[cfg(not(windows))]
fn phase_identity(parent: u32, run: Uuid) -> Option<PhaseIdentity> {
    let matches: Vec<_> = std::fs::read_dir("/proc")
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .filter_map(|pid| {
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
            let fields: Vec<_> = stat.rsplit_once(')')?.1.split_whitespace().collect();
            let cmd = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
            (fields.get(1)?.parse::<u32>().ok()? == parent
                && cmd.windows(36).any(|s| s == run.to_string().as_bytes()))
            .then_some((pid, fields.get(19)?.parse::<u64>().ok()?))
        })
        .collect();
    let [(pid, birth)] = matches.as_slice() else {
        return None;
    };
    let mut frequency = Command::new("getconf");
    frequency.arg("CLK_TCK");
    let hz = observation_output(frequency)?;
    let hz: f64 = std::str::from_utf8(&hz.stdout).ok()?.trim().parse().ok()?;
    let uptime: f64 = std::fs::read_to_string("/proc/uptime")
        .ok()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    Some(PhaseIdentity {
        pid: *pid,
        birth: *birth,
        run,
        started: std::time::Instant::now()
            - std::time::Duration::from_secs_f64((uptime - *birth as f64 / hz).max(0.0)),
    })
}
impl PhaseIdentity {
    #[cfg(windows)]
    fn observed_exit(&self, end: std::time::Instant) -> Option<std::time::Instant> {
        use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
        // Project the OS exit record, not PowerShell shutdown/pipe-drain time.
        // The upper projection includes sampling uncertainty and millisecond
        // quantization; the original verdict deadline is never extended.
        let sampled = Instant::now();
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis();
        let uncertainty = sampled.elapsed() + Duration::from_millis(1);
        let wall_end = wall + end.saturating_duration_since(sampled).as_millis();
        let script = format!(
            "$ErrorActionPreference='Stop'; try {{ $p=[Diagnostics.Process]::GetProcessById({}); $null=$p.Handle; if ($p.StartTime.ToUniversalTime().Ticks -ne {}) {{ exit 1 }}; $ms=[Math]::Max(0,{}-[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()); if (!$p.WaitForExit([int]$ms)) {{ exit 2 }}; [Console]::WriteLine(([DateTimeOffset]$p.ExitTime.ToUniversalTime()).ToUnixTimeMilliseconds()) }} catch [ArgumentException] {{ exit 1 }}",
            self.pid, self.birth, wall_end
        );
        // Existing three-second helper settlement bound, separate from the
        // product timestamp verdict; missing/late evidence remains failure.
        let output = observation_output_until(powershell(&script), end + Duration::from_secs(3))?;
        let exited: u128 = std::str::from_utf8(&output.stdout)
            .ok()?
            .trim()
            .parse()
            .ok()?;
        let at = if exited >= wall {
            sampled.checked_add(Duration::from_millis(u64::try_from(exited - wall).ok()?))?
        } else {
            sampled.checked_sub(Duration::from_millis(u64::try_from(wall - exited).ok()?))?
        };
        at.checked_add(uncertainty)
    }
    #[cfg(not(windows))]
    fn observed_exit(&self, end: std::time::Instant) -> Option<std::time::Instant> {
        observe_child_exit(self.pid, end).then(std::time::Instant::now)
    }
    #[cfg(windows)]
    fn cleanup(&self) {
        let script = format!(
            "try {{ $p=[Diagnostics.Process]::GetProcessById({}); $null=$p.Handle; if ($p.StartTime.ToUniversalTime().Ticks -eq {} -and !$p.HasExited) {{ $p.Kill(); $null=$p.WaitForExit(2000) }} }} catch [ArgumentException] {{ }}",
            self.pid, self.birth
        );
        let _ = observation_output(powershell(&script));
    }
    #[cfg(not(windows))]
    fn cleanup(&self) {
        // pidfd identifies the opened process even if /proc disappears or the
        // numeric PID is reused. No signal is sent until birth/run both match.
        let script = r#"import os,signal,sys,pathlib,select
try:
 fd=os.pidfd_open(int(sys.argv[1]));p=pathlib.Path('/proc')/sys.argv[1]
 stat=(p/'stat').read_text().rsplit(')',1)[1].split()
 if stat[19]==sys.argv[2] and sys.argv[3].encode() in (p/'cmdline').read_bytes():
  signal.pidfd_send_signal(fd,signal.SIGKILL);select.select([fd],[],[],2)
 os.close(fd)
except (ProcessLookupError,FileNotFoundError): pass
"#;
        let mut c = Command::new("python3");
        c.args([
            "-c",
            script,
            &self.pid.to_string(),
            &self.birth.to_string(),
            &self.run.to_string(),
        ]);
        let _ = observation_output(c);
    }
}
/// Existing observers retain their public shape; already-exited children are
/// explicit absence in the owning test, never proof of a deadline/STOP verdict.
pub fn phase_child(parent: u32, run: Uuid) -> (u32, std::time::Instant) {
    let p = phase_identity(parent, run).expect("one owned phase child");
    (p.pid, p.started)
}
/// Custody for the affected real-run tests only. Product verdicts are sampled
/// before Drop; cleanup itself never satisfies a termination assertion.
pub struct OwnedRun {
    parent: std::process::Child,
    phase: Option<PhaseIdentity>,
    run: Uuid,
    files: Vec<std::path::PathBuf>,
    control_root: Option<std::path::PathBuf>,
}
impl OwnedRun {
    pub fn new(parent: std::process::Child, run: Uuid) -> Self {
        Self {
            parent,
            phase: None,
            run,
            files: Vec::new(),
            control_root: None,
        }
    }
    pub fn own_file(&mut self, path: &std::path::Path) {
        self.files.push(path.to_owned());
    }
    pub fn own_control_root(&mut self, path: &std::path::Path) {
        assert_eq!(path.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("dw-provider-control-")
        );
        self.control_root = Some(path.to_owned());
    }
    pub fn observe_phase_exit(&mut self, end: std::time::Instant) -> Option<std::time::Instant> {
        self.phase()?;
        self.phase.as_ref()?.observed_exit(end)
    }
    pub fn phase(&mut self) -> Option<(u32, std::time::Instant)> {
        if self.phase.is_none() {
            self.phase = phase_identity(self.parent.id(), self.run);
        }
        self.phase.as_ref().map(|p| (p.pid, p.started))
    }
}
impl std::ops::Deref for OwnedRun {
    type Target = std::process::Child;
    fn deref(&self) -> &Self::Target {
        &self.parent
    }
}
impl std::ops::DerefMut for OwnedRun {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.parent
    }
}
impl Drop for OwnedRun {
    fn drop(&mut self) {
        let _ = self.phase();
        if let Some(phase) = &self.phase {
            phase.cleanup();
        }
        if !matches!(self.parent.try_wait(), Ok(Some(_))) {
            let _ = self.parent.kill();
        }
        let _ = self.parent.wait();
        for path in &self.files {
            let _ = std::fs::remove_file(path);
        }
        if let Some(root) = &self.control_root {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

/// Returns product termination evidence. A timeout is reported before custodian cleanup.
#[cfg(windows)]
pub fn observe_child_exit(pid: u32, end: std::time::Instant) -> bool {
    let wall_end = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        + end
            .saturating_duration_since(std::time::Instant::now())
            .as_millis();
    let script = format!(
        "$ErrorActionPreference='Stop'; try {{ $p=[Diagnostics.Process]::GetProcessById({pid}); $ms=[Math]::Max(0,{wall_end}-[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()); if ($p.WaitForExit([int]$ms)) {{ 'exited' }} else {{ 'alive' }} }} catch [ArgumentException] {{ 'exited' }}"
    );
    let output = Command::new(
        std::path::Path::new(&std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe"),
    )
    .args(["-NoProfile", "-NonInteractive", "-Command", &script])
    .output()
    .unwrap();
    output.status.success() && output.stdout.starts_with(b"exited")
}
#[cfg(not(windows))]
pub fn observe_child_exit(pid: u32, end: std::time::Instant) -> bool {
    loop {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
        if stat.is_empty()
            || stat
                .rsplit_once(')')
                .is_some_and(|(_, s)| s.trim_start().starts_with('Z'))
        {
            return true;
        }
        if std::time::Instant::now() >= end {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
pub fn cli(command: &str, flag: &str, value: &str) -> (bool, Value) {
    let mut c = self::command();
    let o = c.arg(command).arg(flag).arg(value).output().unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    let receipt = text
        .lines()
        .find(|line| line.starts_with('{'))
        .map(|line| serde_json::from_str(line).unwrap())
        .unwrap_or_else(|| json!({"diagnostic":text.trim()}));
    (o.status.success(), receipt)
}

pub fn path(q: &Value) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("dw-run-{}.json", q["run_id"].as_str().unwrap()));
    std::fs::write(&path, serde_json::to_vec(q).unwrap()).unwrap();
    path
}
pub fn control_root() -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "dw-provider-control-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    #[cfg(windows)]
    {
        use std::process::Stdio;
        let system = std::env::var_os("SystemRoot").unwrap();
        let script = "$ErrorActionPreference='Stop'; \
            $s=[Security.Principal.WindowsIdentity]::GetCurrent().User; \
            $a=New-Object Security.AccessControl.DirectorySecurity; $a.SetOwner($s); \
            $a.SetAccessRuleProtection($true,$false); \
            $r=New-Object Security.AccessControl.FileSystemAccessRule($s,'FullControl','ContainerInherit,ObjectInherit','None','Allow'); \
            $a.AddAccessRule($r); Set-Acl -LiteralPath $env:DW_M1_RUN_CONTROL_ROOT -AclObject $a";
        let status = Command::new(
            std::path::Path::new(&system).join("System32/WindowsPowerShell/v1.0/powershell.exe"),
        )
        .env_clear()
        .env("SystemRoot", system)
        .env("DW_M1_RUN_CONTROL_ROOT", &root)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
        assert!(
            status.success(),
            "private test control root ACL prerequisite"
        );
    }
    root
}
pub fn id(q: &Value) -> Uuid {
    Uuid::parse_str(q["run_id"].as_str().unwrap()).unwrap()
}
pub const PRODUCTION_LEASE_MS: u64 = 2000;
pub const LEASE_RENEW_CADENCE: std::time::Duration = std::time::Duration::from_millis(400);

pub fn release() -> String {
    static RELEASE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    RELEASE
        .get_or_init(|| {
            use sha2::{Digest, Sha256};
            Sha256::digest(std::fs::read(actual_binary()).unwrap())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect()
        })
        .clone()
}
pub fn broker_store() -> duskweave::postgres_m1_run::PgRunStore {
    let mut client = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    duskweave::m1_reasoning::postgres::qualify_owner(&mut client, "dw_m1_broker").unwrap();
    duskweave::postgres_m1_run::PgRunStore::new(client)
}
pub fn broker_store_through(port: u16) -> duskweave::postgres_m1_run::PgRunStore {
    let mut client = terrain::terrain_support::through("DW_INFERENCE_BROKER_DATABASE_URL", port);
    duskweave::m1_reasoning::postgres::qualify_owner(&mut client, "dw_m1_broker").unwrap();
    duskweave::postgres_m1_run::PgRunStore::new(client)
}
pub fn start_with(
    store: &mut duskweave::postgres_m1_run::PgRunStore,
    q: &Value,
) -> duskweave::m1_run::RunRecord {
    let q: duskweave::m1_run::RunRequest = serde_json::from_value(q.clone()).unwrap();
    assert!(store.start(&q, &release()).unwrap());
    store.inspect(q.run_id).unwrap()
}
pub fn start_only(q: &Value) -> duskweave::m1_run::RunRecord {
    let mut store = broker_store();
    start_with(&mut store, q)
}

pub struct LeaseCustodian {
    stop: std::sync::mpsc::Sender<()>,
    worker: Option<std::thread::JoinHandle<duskweave::Res<usize>>>,
}
impl LeaseCustodian {
    pub fn start(
        record: &duskweave::m1_run::RunRecord,
        mut store: duskweave::postgres_m1_run::PgRunStore,
    ) -> Self {
        let run_id = record.request.run_id;
        let deadline = run_deadline(record);
        let initial_next = (std::time::Instant::now() + LEASE_RENEW_CADENCE).min(deadline);
        store.renew(run_id).unwrap();
        let (stop, rx) = std::sync::mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("test_run_lease_custodian".into())
            .spawn(move || renew_loop(store, run_id, deadline, initial_next, rx))
            .unwrap();
        Self {
            stop,
            worker: Some(worker),
        }
    }
    pub fn finish(mut self) -> duskweave::Res<usize> {
        let _ = self.stop.send(());
        self.worker
            .take()
            .expect("custodian join handle")
            .join()
            .expect("custodian thread")
    }
}
impl Drop for LeaseCustodian {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn run_deadline(record: &duskweave::m1_run::RunRecord) -> std::time::Instant {
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock_unavailable")
        .as_millis();
    let remain = u128::try_from(record.deadline_at_ms)
        .ok()
        .and_then(|d| d.checked_sub(now))
        .filter(|d| {
            *d <= if record.request.successor() {
                400000
            } else {
                150000
            }
        })
        .expect("run_deadline");
    Instant::now() + Duration::from_millis(remain as u64)
}
fn renew_loop(
    mut store: duskweave::postgres_m1_run::PgRunStore,
    run_id: Uuid,
    deadline: std::time::Instant,
    mut next: std::time::Instant,
    rx: std::sync::mpsc::Receiver<()>,
) -> duskweave::Res<usize> {
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Instant;
    let mut renewals = 1usize;
    loop {
        match rx.recv_timeout(next.saturating_duration_since(Instant::now())) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => return Ok(renewals),
            Err(RecvTimeoutError::Timeout) => {
                if Instant::now() >= deadline {
                    return Err(duskweave::Fail::State("run_deadline"));
                }
                next = (Instant::now() + LEASE_RENEW_CADENCE).min(deadline);
                store.renew(run_id)?;
                renewals += 1;
            }
        }
    }
}

pub fn publish_first(record: &duskweave::m1_run::RunRecord) {
    use duskweave::{m1_run::RunPhase, postgres_m1_run::PgRunStore};
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let r = record.request.run_id;
    let mut store = PgRunStore::new(native::client("DW_INFERENCE_BROKER_DATABASE_URL"));
    store.renew(r).unwrap();
    store.start_phase(r, RunPhase::DecisionOne).unwrap();
    let mut child = command()
        .args([
            "m1-run-phase",
            "--run",
            &r.to_string(),
            "--phase",
            "decision_one",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("first phase deadline");
        }
        store.renew(r).unwrap();
        std::thread::sleep(Duration::from_millis(100));
    }
    store.renew(r).unwrap();
}

pub fn complete_first(record: &duskweave::m1_run::RunRecord) {
    publish_first(record);
    broker_store()
        .complete_phase(
            record.request.run_id,
            duskweave::m1_run::RunPhase::DecisionOne,
        )
        .unwrap();
}

pub fn timed_conn(variable: &str) -> postgres::Client {
    let mut c = native::client(variable);
    c.batch_execute("SET statement_timeout='3000ms'; SET lock_timeout='750ms'")
        .unwrap();
    c
}
pub fn timed_broker(serializable: bool) -> (duskweave::postgres_m1_run::PgRunStore, i32) {
    let mut c = timed_conn("DW_INFERENCE_BROKER_DATABASE_URL");
    duskweave::m1_reasoning::postgres::qualify_owner(&mut c, "dw_m1_broker").unwrap();
    if serializable {
        c.batch_execute("SET default_transaction_isolation='serializable'")
            .unwrap();
    }
    let pid = c.query_one("SELECT pg_backend_pid()", &[]).unwrap().get(0);
    (duskweave::postgres_m1_run::PgRunStore::new(c), pid)
}
pub fn scope_key(rec: &duskweave::m1_run::RunRecord) -> String {
    let q = &rec.request.context.mission;
    format!("{}:{}", q.engagement_id.0, q.campaign_id.0)
}
pub fn lock_scope(c: &mut postgres::Client, key: &str) {
    c.execute("SELECT pg_advisory_lock(hashtextextended($1,0))", &[&key])
        .unwrap();
}
pub fn unlock_scope(c: &mut postgres::Client, key: &str) {
    let ok: bool = c
        .query_one("SELECT pg_advisory_unlock(hashtextextended($1,0))", &[&key])
        .unwrap()
        .get(0);
    assert!(ok);
}
pub fn wait_advisory_blocked(admin: &mut postgres::Client, pid: i32) {
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + Duration::from_millis(1200);
    loop {
        let blocked: bool = admin
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_locks WHERE pid=$1 AND locktype='advisory' AND NOT granted)",
                &[&pid],
            )
            .unwrap()
            .get(0);
        if blocked {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "advisory lock barrier not reached for backend {pid}"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
pub fn ready_acquisition(
    slot: u128,
) -> (
    duskweave::m1_run::RunRecord,
    duskweave::postgres_m1_run::PgRunStore,
    LeaseCustodian,
) {
    let q = registered(slot);
    let (mut store, _) = timed_broker(false);
    let (custodian_store, _) = timed_broker(false);
    let rec = start_with(&mut store, &q);
    let custodian = LeaseCustodian::start(&rec, custodian_store);
    complete_first(&rec);
    store
        .start_phase(rec.request.run_id, duskweave::m1_run::RunPhase::Acquisition)
        .unwrap();
    store
        .claim_phase(
            rec.request.run_id,
            duskweave::m1_run::RunPhase::Acquisition,
            &release(),
        )
        .unwrap();
    (rec, store, custodian)
}
pub fn withdraw_scope(
    rec: &duskweave::m1_run::RunRecord,
    client: postgres::Client,
) -> duskweave::Res<()> {
    use duskweave::{
        mission::{OperationId, OperatorRef},
        withdrawal::{WithdrawalReason, WithdrawalRequest, WithdrawalStore},
    };
    let q = &rec.request.context.mission;
    let request = WithdrawalRequest {
        engagement_id: q.engagement_id,
        campaign_id: q.campaign_id,
        operator_ref: OperatorRef(Uuid::from_u128(0x11)),
        expected_mission_revision: 1,
        reason: WithdrawalReason::OperatorRequested,
    };
    let mut allocator =
        duskweave::postgres_mission::PgAllocator::new(timed_conn("DW_TEST_DATABASE_URL"));
    let mut mission = duskweave::postgres_mission::PgMissionStore::new(client);
    mission
        .withdraw(&request, OperationId(consent_id()), false, &mut allocator)?
        .ok_or(duskweave::Fail::State("withdrawal_missing"))?;
    Ok(())
}
pub fn fence_state(admin: &mut postgres::Client, rec: &duskweave::m1_run::RunRecord) -> Value {
    let r = rec.request.run_id;
    let row = admin
        .query_one(
            "SELECT y.status, y.lease_until_ms, \
             floor(extract(epoch FROM clock_timestamp())*1000)::bigint, \
             f.phase, (f.operation_id=x.attempt), \
             (SELECT count(*) FROM execution.fixture_dns_attempts WHERE run_id=$1), \
             (SELECT count(*) FROM execution.fixture_dns_results WHERE run_id=$1), \
             (SELECT count(*) FROM mission.withdrawals WHERE (engagement_id,campaign_id)=(x.e,x.c)) \
             FROM execution.autonomous_runs x \
             JOIN execution.autonomous_control y USING(run_id) \
             JOIN execution.session_fences f ON (f.engagement_id,f.campaign_id)=(x.e,x.c) \
             WHERE x.run_id=$1",
            &[&r],
        )
        .unwrap();
    json!({
        "status": row.get::<_, String>(0),
        "lease_current": row.get::<_, i64>(1) > row.get::<_, i64>(2),
        "session_phase": row.get::<_, String>(3),
        "session_attempt_match": row.get::<_, Option<bool>>(4),
        "attempts": row.get::<_, i64>(5),
        "results": row.get::<_, i64>(6),
        "withdrawals": row.get::<_, i64>(7),
    })
}

/// Conservative earliest competing stops, projected from the real owner record.
/// Selection of the parent's phase reaper occurs after phase-start commit.
pub struct PhaseBounds {
    pub started: std::time::Instant,
    pub claimed: Option<std::time::Instant>,
    pub earliest_stop: std::time::Instant,
    pub uncertainty: std::time::Duration,
}
pub fn phase_bounds(
    client: &mut postgres::Client,
    run: Uuid,
    ceiling_ms: u64,
) -> Option<PhaseBounds> {
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    let sampled = Instant::now();
    let row=client.query_opt("SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint-p.recorded_at_ms, x.deadline_at_ms-floor(extract(epoch FROM clock_timestamp())*1000)::bigint, x.deadline_at_ms, (SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint-y.recorded_at_ms FROM execution.autonomous_phases y WHERE y.run_id=x.run_id AND y.phase=p.phase AND y.status='claimed') FROM execution.autonomous_runs x JOIN execution.autonomous_phases p USING(run_id) WHERE x.run_id=$1 AND p.phase='decision_one' AND p.status='started'",&[&run]).unwrap_or_else(|_|panic!("restricted phase observation unavailable"))?;
    let uncertainty = sampled.elapsed();
    let age: u64 = u64::try_from(row.get::<_, i64>(0)).unwrap();
    let remaining = u64::try_from(row.get::<_, i64>(1)).unwrap_or(0);
    let wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let local = u64::try_from(row.get::<_, i64>(2) - wall).unwrap_or(0);
    let started = sampled - Duration::from_millis(age);
    let claimed = row
        .get::<_, Option<i64>>(3)
        .and_then(|n| u64::try_from(n).ok())
        .map(|n| sampled - Duration::from_millis(n));
    Some(PhaseBounds {
        started,
        claimed,
        uncertainty,
        earliest_stop: (started + Duration::from_millis(ceiling_ms))
            .min(sampled + Duration::from_millis(remaining))
            .min(Instant::now() + Duration::from_millis(local)),
    })
}
