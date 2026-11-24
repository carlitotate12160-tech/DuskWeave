#![allow(dead_code)]
//! Platform-neutral HTTPS LAB topology contracts, preflight/privilege validation, and fault-target guards.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(super) const LAB_NETNS: &str = "dw-https-lab-r1";
pub(super) const HOST_VETH: &str = "dwhlab1";
pub(super) const PEER_VETH: &str = "dwslab1";
pub(super) const RESPONDER_UID: u32 = 65531;
pub(super) const RESPONDER_GID: u32 = 65531;
pub(super) const RUNTIME_ROOT: &str = "/run/duskweave-https-lab-r1";
pub(super) const SHARED_CORE_CONTROL_ROOT: &str = "/run/duskweave/m1";
pub(super) const LAUNCHER_ROOT: &str = "/run/duskweave/launcher";
pub(super) const IP_BIN: &str = "/usr/bin/ip";
pub(super) const SETPRIV_BIN: &str = "/usr/bin/setpriv";
pub(super) const SYSCTL_BIN: &str = "/usr/sbin/sysctl";
pub(super) const IPTABLES_BIN: &str = "/usr/sbin/iptables";

#[derive(Clone, Debug, Default)]
pub(super) struct CmdOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub(super) trait CmdRunner {
    fn run(&mut self, bin: &str, args: &[&str]) -> Result<CmdOutput, String>;
}

pub(super) trait ResponderHandle {
    fn id(&self) -> u32;
    fn kill(&mut self) -> std::io::Result<()>;
    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>>;
}

pub(super) fn terminate_and_reap(
    handle: &mut dyn ResponderHandle,
    timeout: Duration,
) -> Result<(), String> {
    match handle.try_wait() {
        Ok(Some(_)) => return Ok(()),
        Ok(None) => {}
        Err(e) => return Err(format!("responder try_wait failed: {e}")),
    }
    let kill_err = handle
        .kill()
        .err()
        .map(|e| format!("responder kill failed: {e}"));
    let deadline = Instant::now() + timeout;
    loop {
        match handle.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(None) => {
                return Err(match kill_err {
                    Some(ke) => format!("{ke}; responder reap timeout"),
                    None => "responder reap timeout".into(),
                });
            }
            Err(e) => {
                return Err(match kill_err {
                    Some(ke) => format!("{ke}; responder try_wait failed: {e}"),
                    None => format!("responder try_wait failed: {e}"),
                });
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct PreflightInputs {
    pub netns_paths_exist: Vec<String>,
    pub ip_netns_list: String,
    pub sys_net_exists: Vec<String>,
    pub ip_link_show: String,
    pub ip_route_all: String,
    pub ip_addr_show: String,
    pub iptables_docker_user: String,
    pub etc_passwd: String,
    pub etc_group: String,
    pub proc_uids_gids: Vec<(u32, u32)>,
    pub docker_ps_names: String,
    pub launcher_entries: Vec<String>,
    pub active_dw_procs: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ProcStatusSummary {
    pub uids: Vec<u32>,
    pub gids: Vec<u32>,
    pub is_zombie: bool,
    pub has_vm_size: bool,
    pub is_kthread: bool,
}

#[derive(Clone, Debug, Default)]
pub(super) struct OwnedTopologyState {
    pub netns: bool,
    pub host_veth: bool,
    pub route_dns: bool,
    pub route_https: bool,
    pub iptables_rules: Vec<u8>,
}

impl OwnedTopologyState {
    pub fn has_any(&self) -> bool {
        self.netns
            || self.host_veth
            || self.route_dns
            || self.route_https
            || !self.iptables_rules.is_empty()
    }
}

#[derive(Clone, Debug)]
pub(super) struct RuntimeDirs {
    pub root: PathBuf,
    pub instance_dir: PathBuf,
    pub responder_dir: PathBuf,
    pub counter_path: PathBuf,
    pub core_control_dir: PathBuf,
    pub core_input_dir: PathBuf,
    pub faulted_control_dir: Option<PathBuf>,
    pub root_created: bool,
}

impl RuntimeDirs {
    pub fn new_under(root: &Path, tag: &str, root_created: bool) -> Self {
        let instance_dir = root.join(tag);
        let responder_dir = instance_dir.join("responder");
        Self {
            root: root.to_path_buf(),
            counter_path: responder_dir.join("https-counters.json"),
            core_control_dir: instance_dir.join("core-control"),
            core_input_dir: instance_dir.join("core-inputs"),
            instance_dir,
            responder_dir,
            faulted_control_dir: None,
            root_created,
        }
    }

    pub fn validate_control_fault_target(&self, suffix: &str) -> Result<PathBuf, String> {
        let valid_suffix = !suffix.is_empty()
            && suffix
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if self.faulted_control_dir.is_some() || !valid_suffix {
            return Err("invalid_control_fault_state".into());
        }
        if self.core_control_dir == Path::new(SHARED_CORE_CONTROL_ROOT)
            || self.core_control_dir.parent() != Some(&self.instance_dir)
        {
            return Err("refusing_non_owned_control_root".into());
        }
        Ok(self
            .instance_dir
            .join(format!("core-control.fault-{suffix}")))
    }

    pub fn inject_control_root_loss(&mut self, suffix: &str) -> Result<PathBuf, String> {
        let sibling = self.validate_control_fault_target(suffix)?;
        let meta = std::fs::symlink_metadata(&self.core_control_dir).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink()
            || !meta.is_dir()
            || std::fs::symlink_metadata(&sibling).is_ok()
        {
            return Err("control_root_fault_precondition_failed".into());
        }
        std::fs::rename(&self.core_control_dir, &sibling).map_err(|e| e.to_string())?;
        self.faulted_control_dir = Some(sibling.clone());
        Ok(sibling)
    }
}

pub(super) fn is_duskweave_proc_exe(exe_path: &str) -> bool {
    matches!(
        exe_path,
        "/usr/local/libexec/duskweave/duskweave"
            | "/usr/local/libexec/duskweave/dw-boundary-launcher"
    ) || exe_path.ends_with("/httpslab-arm64")
}

pub(super) fn parse_proc_status_summary(status: &str) -> Result<ProcStatusSummary, String> {
    let mut fields = BTreeMap::new();
    for line in status.lines() {
        if let Some((key, val)) = line.split_once(':') {
            fields.insert(key.trim(), val.trim());
        }
    }
    let parse_ids = |key: &str| -> Result<Vec<u32>, String> {
        let raw = fields
            .get(key)
            .ok_or_else(|| format!("missing {key} in /proc status"))?;
        let mut parsed = Vec::new();
        for tok in raw.split_whitespace() {
            parsed.push(
                tok.parse::<u32>()
                    .map_err(|_| format!("invalid {key} token in /proc status: {tok}"))?,
            );
        }
        if parsed.len() < 4 {
            return Err(format!("truncated {key} fields in /proc status"));
        }
        Ok(parsed)
    };
    let uids = parse_ids("Uid")?;
    let gids = parse_ids("Gid")?;
    let is_zombie = fields.get("State").is_some_and(|val| val.starts_with('Z'));
    let has_vm_size = fields.contains_key("VmSize");
    let is_kthread = fields.get("Kthread").copied() == Some("1");
    Ok(ProcStatusSummary {
        uids,
        gids,
        is_zombie,
        has_vm_size,
        is_kthread,
    })
}

pub(super) fn check_preflight_collisions(inp: &PreflightInputs) -> Result<(), String> {
    let ns_hit = inp.ip_netns_list.lines().any(|line| {
        line.split_whitespace()
            .next()
            .is_some_and(|tok| tok == LAB_NETNS || tok.starts_with("dw"))
    });
    if !inp.netns_paths_exist.is_empty() || ns_hit {
        return Err("netns collision".into());
    }
    if !inp.sys_net_exists.is_empty()
        || [HOST_VETH, PEER_VETH]
            .iter()
            .any(|iface| inp.ip_link_show.contains(iface))
    {
        return Err("veth collision".into());
    }
    let route_hit = ["10.204.0.", "192.0.2.53", "198.51.100.10"]
        .iter()
        .any(|needle| inp.ip_route_all.contains(needle));
    let addr_hit = ["10.204.0.1", "10.204.0.2", "192.0.2.53", "198.51.100.10"]
        .iter()
        .any(|needle| inp.ip_addr_show.contains(needle));
    if route_hit || addr_hit {
        return Err("route/addr collision".into());
    }
    if [HOST_VETH, LAB_NETNS]
        .iter()
        .any(|needle| inp.iptables_docker_user.contains(needle))
    {
        return Err("iptables collision".into());
    }
    let has_passwd_id = |table: &str, target_id: u32| {
        table
            .lines()
            .any(|line| line.split(':').nth(2) == Some(&target_id.to_string()))
    };
    if has_passwd_id(&inp.etc_passwd, RESPONDER_UID)
        || has_passwd_id(&inp.etc_group, RESPONDER_GID)
        || inp
            .proc_uids_gids
            .iter()
            .any(|&(uid, gid)| uid == RESPONDER_UID || gid == RESPONDER_GID)
    {
        return Err("uid/gid 65531 collision".into());
    }
    let container_hit = inp
        .docker_ps_names
        .lines()
        .map(str::trim)
        .any(|name| name.starts_with("dwcap") || name.starts_with("dwexp"));
    if container_hit || !inp.launcher_entries.is_empty() || !inp.active_dw_procs.is_empty() {
        return Err("active duskweave work".into());
    }
    Ok(())
}

pub(super) fn resolve_core_gid_from_passwd(passwd: &str, core_uid: u32) -> Result<u32, String> {
    for line in passwd.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 4
            && parts[0] == "duskweave-core"
            && parts[2].parse::<u32>().ok() == Some(core_uid)
        {
            return parts[3]
                .parse::<u32>()
                .map_err(|_| "invalid_core_gid".into());
        }
    }
    Err(format!("duskweave-core uid {core_uid} missing"))
}

pub(super) fn verify_proc_status(status: &str, uid: u32, gid: u32) -> Result<(), String> {
    let mut fields = BTreeMap::new();
    for line in status.lines() {
        if let Some((key, val)) = line.split_once(':') {
            fields.insert(key.trim(), val.trim());
        }
    }
    for (key, expected) in [("Uid", uid), ("Gid", gid)] {
        let values: Vec<u32> = fields
            .get(key)
            .ok_or_else(|| format!("missing {key}"))?
            .split_whitespace()
            .filter_map(|tok| tok.parse().ok())
            .collect();
        if values != [expected, expected, expected, expected] {
            return Err(format!("{key} mismatch"));
        }
    }
    if fields.get("Groups").is_none_or(|g| !g.is_empty())
        || fields.get("NoNewPrivs").copied() != Some("1")
    {
        return Err("groups/nnp failed".into());
    }
    for cap in ["CapInh", "CapPrm", "CapEff", "CapBnd", "CapAmb"] {
        if fields.get(cap).copied() != Some("0000000000000000") {
            return Err(format!("{cap} non-zero"));
        }
    }
    Ok(())
}

pub(super) fn iptables_rule_args(op: &str, idx: u8, comment: &str) -> Vec<String> {
    let (proto, cidr, port_flag, port, outbound) = match idx {
        0 => ("udp", "192.0.2.53/32", "--dport", "53", false),
        1 => ("udp", "192.0.2.53/32", "--sport", "53", true),
        2 => ("tcp", "198.51.100.10/32", "--dport", "443", false),
        _ => ("tcp", "198.51.100.10/32", "--sport", "443", true),
    };
    let (in_if, out_if, src, dst) = if outbound {
        (HOST_VETH, "dvh+", cidr, "10.203.0.0/24")
    } else {
        ("dvh+", HOST_VETH, "10.203.0.0/24", cidr)
    };
    let spec = format!(
        "{op} DOCKER-USER -i {in_if} -o {out_if} -s {src} -d {dst} -p {proto} {port_flag} {port} -m comment --comment {comment} -j ACCEPT"
    );
    spec.split_whitespace().map(str::to_string).collect()
}

pub(super) fn build_responder_launch_args(
    responder_bin: &str,
    scenario: &str,
    run_id: Uuid,
    dns_ttl: u32,
    dns_refresh_ttl: u32,
    counter_path: &Path,
) -> Vec<String> {
    let head = format!(
        "netns exec {LAB_NETNS} {SETPRIV_BIN} --reuid={RESPONDER_UID} --regid={RESPONDER_GID} --clear-groups --inh-caps=-all --ambient-caps=-all --bounding-set=-all --no-new-privs --"
    );
    let tail = format!(
        "-listen 198.51.100.10:443 -dns-listen 192.0.2.53:53 -dns-ttl {dns_ttl} -dns-refresh-ttl {dns_refresh_ttl} -scenario {scenario} -run-id {run_id} -counters"
    );
    let mut args: Vec<String> = head.split_whitespace().map(str::to_string).collect();
    args.push(responder_bin.into());
    args.extend(tail.split_whitespace().map(str::to_string));
    args.push(counter_path.to_str().expect("utf8 counter path").into());
    args
}

fn run_spec(runner: &mut dyn CmdRunner, bin: &str, spec: &str) -> Result<(), String> {
    let args: Vec<&str> = spec.split_whitespace().collect();
    let out = runner.run(bin, &args)?;
    out.success
        .then_some(())
        .ok_or_else(|| format!("{bin} {spec}: {}", out.stderr.trim()))
}

pub(super) fn setup_network_with(
    runner: &mut dyn CmdRunner,
    comment: &str,
    owned: &mut OwnedTopologyState,
) -> Result<(), String> {
    run_spec(runner, IP_BIN, "netns add dw-https-lab-r1")?;
    owned.netns = true;
    run_spec(
        runner,
        IP_BIN,
        "link add dwhlab1 type veth peer name dwslab1",
    )?;
    owned.host_veth = true;
    let sysctl_spec = format!(
        "netns exec {LAB_NETNS} {SYSCTL_BIN} -qw net.ipv6.conf.all.disable_ipv6=1 net.ipv6.conf.default.disable_ipv6=1 net.ipv6.conf.lo.disable_ipv6=1 net.ipv4.ip_unprivileged_port_start=0"
    );
    for spec in [
        "link set dwslab1 netns dw-https-lab-r1",
        &sysctl_spec,
        "addr add 10.204.0.1/30 dev dwhlab1",
        "link set dwhlab1 up",
        "netns exec dw-https-lab-r1 /usr/bin/ip addr add 10.204.0.2/30 dev dwslab1",
        "netns exec dw-https-lab-r1 /usr/bin/ip link set dwslab1 up",
        "netns exec dw-https-lab-r1 /usr/bin/ip link set lo up",
        "netns exec dw-https-lab-r1 /usr/bin/ip addr add 192.0.2.53/32 dev lo",
        "netns exec dw-https-lab-r1 /usr/bin/ip addr add 198.51.100.10/32 dev lo",
        "netns exec dw-https-lab-r1 /usr/bin/ip route add 10.203.0.0/24 via 10.204.0.1 dev dwslab1",
    ] {
        run_spec(runner, IP_BIN, spec)?;
    }
    run_spec(
        runner,
        IP_BIN,
        "route add 192.0.2.53/32 via 10.204.0.2 dev dwhlab1",
    )?;
    owned.route_dns = true;
    run_spec(
        runner,
        IP_BIN,
        "route add 198.51.100.10/32 via 10.204.0.2 dev dwhlab1",
    )?;
    owned.route_https = true;
    for idx in 0..4u8 {
        let args = iptables_rule_args("-I", idx, comment);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = runner.run(IPTABLES_BIN, &refs)?;
        if !out.success {
            return Err(format!("iptables -I {idx}: {}", out.stderr.trim()));
        }
        owned.iptables_rules.push(idx);
    }
    Ok(())
}

pub(super) fn dispose_network_with(
    runner: &mut dyn CmdRunner,
    comment: &str,
    owned: &mut OwnedTopologyState,
) -> Result<(), Vec<String>> {
    let (mut errors, mut kept) = (Vec::new(), Vec::new());
    for idx in owned.iptables_rules.iter().copied().rev() {
        let args = iptables_rule_args("-D", idx, comment);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        match runner.run(IPTABLES_BIN, &refs) {
            Ok(out) if out.success => {}
            Ok(out) => {
                errors.push(format!("iptables -D {idx}: {}", out.stderr.trim()));
                kept.push(idx);
            }
            Err(err) => {
                errors.push(err);
                kept.push(idx);
            }
        }
    }
    kept.reverse();
    owned.iptables_rules = kept;
    for (flag, spec) in [
        (
            &mut owned.route_https,
            "route del 198.51.100.10/32 via 10.204.0.2 dev dwhlab1",
        ),
        (
            &mut owned.route_dns,
            "route del 192.0.2.53/32 via 10.204.0.2 dev dwhlab1",
        ),
        (&mut owned.host_veth, "link del dwhlab1"),
        (&mut owned.netns, "netns del dw-https-lab-r1"),
    ] {
        if *flag {
            match run_spec(runner, IP_BIN, spec) {
                Ok(()) => *flag = false,
                Err(err) => errors.push(err),
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub(super) fn acquire_network_with(
    runner: &mut dyn CmdRunner,
    comment: &str,
    owned: &mut OwnedTopologyState,
) -> Result<(), String> {
    if let Err(setup_err) = setup_network_with(runner, comment, owned) {
        if let Err(clean_errs) = dispose_network_with(runner, comment, owned) {
            return Err(format!(
                "{setup_err}; cleanup unresolved: {}",
                clean_errs.join("; ")
            ));
        }
        return Err(setup_err);
    }
    Ok(())
}
