#![cfg(target_os = "linux")]
#![allow(dead_code)]
//! Linux host command execution, preflight inspection, directory custody, and `LabTopology` RAII lifecycle.

use super::topology::{
    CmdOutput, CmdRunner, HOST_VETH, IP_BIN, IPTABLES_BIN, LAB_NETNS, LAUNCHER_ROOT,
    OwnedTopologyState, PEER_VETH, PreflightInputs, RESPONDER_GID, RESPONDER_UID, RUNTIME_ROOT,
    ResponderHandle, RuntimeDirs, acquire_network_with, build_responder_launch_args,
    check_preflight_collisions, dispose_network_with, is_duskweave_proc_exe,
    parse_proc_status_summary, terminate_and_reap, verify_proc_status,
};
use crate::sdk_fixture::FixtureChild;
use rustix::fs::{Gid, Uid, chown};
use std::io::ErrorKind;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(super) type ProcScanResult = (Vec<(u32, u32)>, Vec<String>);

impl ResponderHandle for Child {
    fn id(&self) -> u32 {
        Child::id(self)
    }
    fn kill(&mut self) -> std::io::Result<()> {
        Child::kill(self)
    }
    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        Child::try_wait(self)
    }
}

pub(super) struct OsCmdRunner;

impl CmdRunner for OsCmdRunner {
    fn run(&mut self, bin: &str, args: &[&str]) -> Result<CmdOutput, String> {
        let child = Command::new(bin)
            .args(args)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut fc = FixtureChild::new(child);
        if fc
            .wait_until(Instant::now() + Duration::from_secs(2))
            .is_none()
        {
            return Err(format!("timeout {bin}"));
        }
        let out = fc.output();
        Ok(CmdOutput {
            success: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).into(),
            stderr: String::from_utf8_lossy(&out.stderr).into(),
        })
    }
}

fn query(runner: &mut dyn CmdRunner, bin: &str, args: &[&str]) -> Result<String, String> {
    let out = runner.run(bin, args)?;
    out.success
        .then_some(out.stdout)
        .ok_or_else(|| format!("{bin}: {}", out.stderr.trim()))
}

fn safe_entries(path: &str) -> Result<Vec<String>, String> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
        Ok(m) if m.file_type().is_symlink() || !m.is_dir() => return Err(format!("{path} unsafe")),
        Ok(_) => {}
    }
    std::fs::read_dir(path)
        .map_err(|e| e.to_string())?
        .map(|entry| {
            entry
                .map(|d| d.file_name().to_string_lossy().into())
                .map_err(|e| e.to_string())
        })
        .collect()
}

pub(super) fn scan_proc_state_at(
    proc_root: &Path,
    self_pid: u32,
) -> Result<ProcScanResult, String> {
    let (mut ids, mut procs) = (Vec::new(), Vec::new());
    let entries = std::fs::read_dir(proc_root)
        .map_err(|e| format!("failed reading {}: {e}", proc_root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed iterating {}: {e}", proc_root.display()))?;
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if pid == self_pid {
            continue;
        }
        let pid_dir = entry.path();
        let status = match std::fs::read_to_string(pid_dir.join("status")) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound && !pid_dir.exists() => continue,
            Err(e) => return Err(format!("failed reading /proc/{pid}/status: {e}")),
        };
        let summary = parse_proc_status_summary(&status)?;
        let (uid_hit, gid_hit) = (
            summary.uids.contains(&RESPONDER_UID),
            summary.gids.contains(&RESPONDER_GID),
        );
        if uid_hit || gid_hit {
            ids.push((
                u32::from(uid_hit) * RESPONDER_UID,
                u32::from(gid_hit) * RESPONDER_GID,
            ));
        }
        match std::fs::read_link(pid_dir.join("exe")) {
            Ok(exe) => {
                let exe_str = exe.to_string_lossy();
                if is_duskweave_proc_exe(&exe_str) {
                    procs.push(exe_str.into_owned());
                }
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {
                if !pid_dir.exists() || summary.is_zombie {
                    continue;
                }
                let empty_cmdline = std::fs::read(pid_dir.join("cmdline"))
                    .map(|b| b.is_empty())
                    .unwrap_or(false);
                if summary.is_kthread || (!summary.has_vm_size && empty_cmdline) {
                    continue;
                }
                return Err(format!("unexplained missing exe for live pid {pid}"));
            }
            Err(e) => return Err(format!("failed reading /proc/{pid}/exe: {e}")),
        }
    }
    Ok((ids, procs))
}

pub(super) fn collect_linux_preflight(
    runner: &mut dyn CmdRunner,
) -> Result<PreflightInputs, String> {
    let raw_launcher = safe_entries(LAUNCHER_ROOT)?;
    let launcher_entries = raw_launcher
        .iter()
        .filter(|name| {
            name.strip_suffix(".pending")
                .is_some_and(|stem| !raw_launcher.contains(&format!("{stem}.complete")))
                || (name.len() == 32
                    && name.bytes().all(|b| b.is_ascii_hexdigit())
                    && Path::new(LAUNCHER_ROOT)
                        .join(name)
                        .join("session.json")
                        .exists())
        })
        .cloned()
        .collect();
    let (proc_uids_gids, active_dw_procs) =
        scan_proc_state_at(Path::new("/proc"), std::process::id())?;
    Ok(PreflightInputs {
        netns_paths_exist: safe_entries("/run/netns")?
            .into_iter()
            .filter(|name| name == LAB_NETNS || name.starts_with("dw"))
            .collect(),
        sys_net_exists: [HOST_VETH, PEER_VETH]
            .into_iter()
            .filter(|iface| std::fs::symlink_metadata(format!("/sys/class/net/{iface}")).is_ok())
            .map(str::to_string)
            .collect(),
        launcher_entries,
        proc_uids_gids,
        active_dw_procs,
        ip_netns_list: query(runner, IP_BIN, &["netns", "list"])?,
        ip_link_show: query(runner, IP_BIN, &["link", "show"])?,
        ip_route_all: query(runner, IP_BIN, &["-4", "route", "show", "table", "all"])?,
        ip_addr_show: query(runner, IP_BIN, &["-4", "addr", "show"])?,
        iptables_docker_user: query(runner, IPTABLES_BIN, &["-S", "DOCKER-USER"])?,
        etc_passwd: std::fs::read_to_string("/etc/passwd").map_err(|e| e.to_string())?,
        etc_group: std::fs::read_to_string("/etc/group").map_err(|e| e.to_string())?,
        docker_ps_names: query(
            runner,
            "/usr/bin/docker",
            &["ps", "-a", "--format", "{{.Names}}"],
        )?,
    })
}

fn verify_dir(path: &Path, owner: (u32, u32), mode: u32) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink()
        || !meta.is_dir()
        || (meta.uid(), meta.gid()) != owner
        || (meta.mode() & 0o777) != mode
    {
        return Err(format!("unsafe dir: {}", path.display()));
    }
    Ok(())
}

fn create_owned_dir(
    path: &Path,
    owner: (u32, u32),
    mode: u32,
    created: &mut Vec<PathBuf>,
) -> Result<(), String> {
    std::fs::create_dir(path).map_err(|e| e.to_string())?;
    // Record creation before chown/chmod so rollback removes only newly created dirs.
    created.push(path.to_path_buf());
    chown(
        path,
        Some(Uid::from_raw(owner.0)),
        Some(Gid::from_raw(owner.1)),
    )
    .map_err(|e| e.to_string())?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|e| e.to_string())?;
    verify_dir(path, owner, mode)
}

pub(super) fn rollback_created_dirs(created: &[PathBuf]) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for path in created.iter().rev() {
        match std::fs::remove_dir(path) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => errors.push(format!("rollback {}: {e}", path.display())),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub(super) fn create_scoped_dirs_with_owner(
    root: &Path,
    tag: &str,
    root_owner: (u32, u32),
    core_owner: (u32, u32),
    responder_owner: (u32, u32),
) -> Result<RuntimeDirs, String> {
    for ancestor in root.ancestors().skip(1) {
        let meta = std::fs::symlink_metadata(ancestor).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err(format!("unsafe ancestor: {}", ancestor.display()));
        }
    }
    let mut created = Vec::new();
    let mut dirs = RuntimeDirs::new_under(root, tag, false);
    let build = (|| -> Result<bool, String> {
        let root_created = match std::fs::symlink_metadata(root) {
            Err(e) if e.kind() == ErrorKind::NotFound => {
                create_owned_dir(root, root_owner, 0o711, &mut created)?;
                true
            }
            Err(e) => return Err(e.to_string()),
            Ok(_) => {
                verify_dir(root, root_owner, 0o711)?;
                if std::fs::read_dir(root)
                    .map_err(|e| e.to_string())?
                    .next()
                    .is_some()
                {
                    return Err(format!("non-empty {}", root.display()));
                }
                false
            }
        };
        for (path, owner, mode) in [
            (&dirs.instance_dir, root_owner, 0o711),
            (&dirs.responder_dir, responder_owner, 0o700),
            (&dirs.core_control_dir, core_owner, 0o700),
            (&dirs.core_input_dir, core_owner, 0o700),
        ] {
            create_owned_dir(path, owner, mode, &mut created)?;
        }
        Ok(root_created)
    })();
    match build {
        Ok(root_created) => {
            dirs.root_created = root_created;
            Ok(dirs)
        }
        Err(err) => match rollback_created_dirs(&created) {
            Ok(()) => Err(err),
            Err(rb_errs) => Err(format!("{err}; {}", rb_errs.join("; "))),
        },
    }
}

pub(super) fn cleanup_runtime_dirs(dirs: &mut RuntimeDirs) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for path in dirs.faulted_control_dir.iter().chain([
        &dirs.core_control_dir,
        &dirs.core_input_dir,
        &dirs.responder_dir,
        &dirs.instance_dir,
    ]) {
        match std::fs::symlink_metadata(path) {
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => errors.push(e.to_string()),
            Ok(m) if m.file_type().is_symlink() => {
                errors.push(format!("symlink: {}", path.display()))
            }
            Ok(_) => {
                if let Err(e) = std::fs::remove_dir_all(path) {
                    errors.push(e.to_string());
                }
            }
        }
    }
    if dirs.root_created {
        match std::fs::remove_dir(&dirs.root) {
            Ok(()) => dirs.root_created = false,
            Err(e) if e.kind() == ErrorKind::NotFound => dirs.root_created = false,
            Err(e) => errors.push(e.to_string()),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub(super) fn verify_live_responder(
    pid: u32,
    counter_path: &Path,
    responder_bin: &Path,
) -> Result<(), String> {
    let cm = std::fs::symlink_metadata(counter_path).map_err(|e| e.to_string())?;
    if cm.file_type().is_symlink()
        || !cm.is_file()
        || cm.uid() != RESPONDER_UID
        || (cm.mode() & 0o777) != 0o600
    {
        return Err("counter file unsafe owner/mode".into());
    }
    verify_proc_status(
        &std::fs::read_to_string(format!("/proc/{pid}/status")).map_err(|e| e.to_string())?,
        RESPONDER_UID,
        RESPONDER_GID,
    )?;
    let exe = std::fs::read_link(format!("/proc/{pid}/exe")).map_err(|e| e.to_string())?;
    if exe != responder_bin {
        return Err(format!("responder exe mismatch: {}", exe.display()));
    }
    let proc_ns = std::fs::metadata(format!("/proc/{pid}/ns/net")).map_err(|e| e.to_string())?;
    let lab_ns = std::fs::metadata(format!("/run/netns/{LAB_NETNS}")).map_err(|e| e.to_string())?;
    if (proc_ns.dev(), proc_ns.ino()) != (lab_ns.dev(), lab_ns.ino()) {
        return Err("responder outside dw-https-lab-r1".into());
    }
    Ok(())
}

pub(super) struct LabTopology {
    pub dirs: RuntimeDirs,
    pub(super) rule_comment: String,
    pub(super) owned: OwnedTopologyState,
    pub(super) responder: Option<Box<dyn ResponderHandle>>,
    pub(super) reap_timeout: Duration,
}

impl LabTopology {
    pub fn new(tag: &str, core_uid: u32, core_gid: u32) -> Result<Self, String> {
        check_preflight_collisions(&collect_linux_preflight(&mut OsCmdRunner)?)?;
        let dirs = create_scoped_dirs_with_owner(
            Path::new(RUNTIME_ROOT),
            tag,
            (0, 0),
            (core_uid, core_gid),
            (RESPONDER_UID, RESPONDER_GID),
        )?;
        Ok(Self {
            dirs,
            rule_comment: format!("{LAB_NETNS}:{tag}"),
            owned: OwnedTopologyState::default(),
            responder: None,
            reap_timeout: Duration::from_secs(2),
        })
    }

    pub fn start_with_spawn(
        &mut self,
        runner: &mut dyn CmdRunner,
        spawn: impl FnOnce() -> Result<Box<dyn ResponderHandle>, String>,
        verify_ready: impl FnOnce(u32) -> Result<(), String>,
    ) -> Result<(), String> {
        let launch = (|| -> Result<(), String> {
            acquire_network_with(runner, &self.rule_comment, &mut self.owned)?;
            let handle = spawn()?;
            let pid = handle.id();
            // Transfer custody into self.responder immediately after spawn before any readiness check.
            self.responder = Some(handle);
            let deadline = Instant::now() + Duration::from_secs(3);
            while !self.dirs.counter_path.exists() && Instant::now() < deadline {
                if let Some(child) = self.responder.as_mut()
                    && let Some(status) = child.try_wait().map_err(|e| e.to_string())?
                {
                    self.responder = None;
                    return Err(format!("httpslab exited: {status}"));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            verify_ready(pid)
        })();
        if let Err(err) = launch {
            let mut detail = vec![err];
            if let Err(mut cleanup_errs) = self.dispose_with(runner) {
                detail.append(&mut cleanup_errs);
            }
            return Err(detail.join("; "));
        }
        Ok(())
    }

    pub fn start_for(
        &mut self,
        responder_bin: &Path,
        scenario: &str,
        run_id: Uuid,
        dns_ttl: u32,
        dns_refresh_ttl: u32,
    ) -> Result<(), String> {
        let bin_str = responder_bin.to_str().ok_or("non-utf8 responder path")?;
        let args = build_responder_launch_args(
            bin_str,
            scenario,
            run_id,
            dns_ttl,
            dns_refresh_ttl,
            &self.dirs.counter_path,
        );
        let counter_path = self.dirs.counter_path.clone();
        self.start_with_spawn(
            &mut OsCmdRunner,
            || {
                let child = Command::new(IP_BIN)
                    .args(&args)
                    .env_clear()
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| e.to_string())?;
                Ok(Box::new(child))
            },
            |pid| verify_live_responder(pid, &counter_path, responder_bin),
        )
    }

    pub fn dispose_with(&mut self, runner: &mut dyn CmdRunner) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if let Some(mut handle) = self.responder.take()
            && let Err(err) = terminate_and_reap(handle.as_mut(), self.reap_timeout)
        {
            errors.push(err);
            self.responder = Some(handle);
        }
        if let Err(mut net_errs) = dispose_network_with(runner, &self.rule_comment, &mut self.owned)
        {
            errors.append(&mut net_errs);
        }
        if let Err(mut dir_errs) = cleanup_runtime_dirs(&mut self.dirs) {
            errors.append(&mut dir_errs);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn dispose(&mut self) -> Result<(), Vec<String>> {
        self.dispose_with(&mut OsCmdRunner)
    }
}

impl Drop for LabTopology {
    fn drop(&mut self) {
        if (self.responder.is_some()
            || self.owned.has_any()
            || self.dirs.instance_dir.exists()
            || self.dirs.root_created)
            && let Err(errors) = self.dispose()
        {
            if std::thread::panicking() {
                eprintln!("unresolved LAB cleanup: {}", errors.join("; "));
            } else {
                panic!("scoped LAB disposal incomplete: {}", errors.join("; "));
            }
        }
    }
}
