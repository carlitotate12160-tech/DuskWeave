//! Scoped process identity, observation and cleanup for real-run fixtures.
use std::process::Command;
use uuid::Uuid;

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
