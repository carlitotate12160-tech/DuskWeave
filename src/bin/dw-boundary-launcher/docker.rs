//! Docker lifecycle for the attempt pair: dedicated bridge network, the full
//! ADR-015 isolation flag set, pipe plumbing, bounded kill/reap and disposal.
//! The launcher is the only client; workers expose no shell or generic API.
use std::process::{Child, Command, Stdio};

pub const CAPTURE_UID: u32 = 65532;
pub const EXPORTER_UID: u32 = 65533;
const MEM_LIMIT: &str = "128m";
const CPU_LIMIT: &str = "0.5";
const PID_LIMIT: &str = "32";
const FD_LIMIT: &str = "64";

pub struct Spawned {
    pub name: String,
    pub child: Child,
}

fn docker(args: &[String]) -> Result<std::process::Output, &'static str> {
    Command::new("docker")
        .args(args)
        .output()
        .map_err(|_| "docker_invoke_failed")
}

fn ok(args: &[String]) -> Result<String, &'static str> {
    let out = docker(args)?;
    if !out.status.success() {
        return Err("docker_command_failed");
    }
    String::from_utf8(out.stdout)
        .map(|s| s.trim().to_string())
        .map_err(|_| "docker_output_invalid")
}

/// Create the attempt-dedicated bridge; forced name lets nft rules match the
/// interface deterministically. Returns the host-side gateway address.
pub fn network_create(tag: &str, session: &str) -> Result<(String, String), &'static str> {
    let net = format!("dw-cap-{tag}");
    let bridge = format!("dwcap{tag}");
    let args = vec![
        "network".into(),
        "create".into(),
        "--driver".into(),
        "bridge".into(),
        "--opt".into(),
        format!("com.docker.network.bridge.name={bridge}"),
        "--label".into(),
        format!("dw.session={session}"),
        net.clone(),
    ];
    let id = match ok(&args) {
        Ok(id) if !id.is_empty() => id,
        // A harness may pre-create the attempt bridge to bind its stub on the
        // gateway before issuing the request; adopt it only if it exists.
        _ if network_exists(&net) => String::new(),
        _ => return Err("docker_command_failed"),
    };
    let _ = id;
    let gw = ok(&vec![
        "network".into(),
        "inspect".into(),
        "-f".into(),
        "{{(index .IPAM.Config 0).Gateway}}".into(),
        net.clone(),
    ])?;
    if gw.is_empty() {
        network_remove(&net);
        return Err("docker_gateway_missing");
    }
    Ok((net, gw))
}

pub fn network_remove(net: &str) {
    let _ = docker(&vec!["network".into(), "rm".into(), net.to_string()]);
}

fn network_exists(net: &str) -> bool {
    docker(&vec!["network".into(), "inspect".into(), net.into()])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn isolation_args(name: &str, session: &str, uid: u32, net: &str, entry: &str) -> Vec<String> {
    vec![
        "run".into(),
        "-i".into(),
        "--rm".into(),
        "--name".into(),
        name.into(),
        "--network".into(),
        net.into(),
        "--read-only".into(),
        "--cap-drop".into(),
        "ALL".into(),
        "--security-opt".into(),
        "no-new-privileges:true".into(),
        "--memory".into(),
        MEM_LIMIT.into(),
        "--memory-swap".into(),
        MEM_LIMIT.into(),
        "--cpus".into(),
        CPU_LIMIT.into(),
        "--pids-limit".into(),
        PID_LIMIT.into(),
        "--ulimit".into(),
        "core=0:0".into(),
        "--ulimit".into(),
        format!("nofile={FD_LIMIT}:{FD_LIMIT}"),
        "--log-driver".into(),
        "none".into(),
        "--user".into(),
        uid.to_string(),
        "--label".into(),
        format!("dw.session={session}"),
        "--entrypoint".into(),
        entry.into(),
    ]
}

/// Spawn one fixed-entrypoint worker with piped stdio; stderr is discarded.
pub fn spawn(
    role: &str,
    tag: &str,
    session: &str,
    uid: u32,
    net: &str,
    image: &str,
) -> Result<Spawned, &'static str> {
    let name = format!("dw-{role}-{tag}");
    let entry = format!("/{role}");
    let mut args = isolation_args(&name, session, uid, net, &entry);
    args.push(image.into());
    let child = Command::new("docker")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "docker_invoke_failed")?;
    Ok(Spawned { name, child })
}

/// SIGKILL then bounded wait; the caller measures the <=1s bound.
pub fn kill(name: &str) {
    let _ = docker(&vec![
        "kill".into(),
        "-s".into(),
        "KILL".into(),
        name.into(),
    ]);
}

/// Remove any leftover objects bearing the session label (crash sweep).
pub fn sweep(session: &str) -> Result<u32, &'static str> {
    let filter = format!("label=dw.session={session}");
    let ids = ok(&vec![
        "ps".into(),
        "-aq".into(),
        "--filter".into(),
        filter.clone(),
    ])?;
    let mut removed = 0u32;
    for id in ids.lines().filter(|l| !l.is_empty()) {
        let _ = docker(&vec!["rm".into(), "-f".into(), id.to_string()]);
        removed += 1;
    }
    let nets = ok(&vec![
        "network".into(),
        "ls".into(),
        "-q".into(),
        "--filter".into(),
        filter,
    ])?;
    for id in nets.lines().filter(|l| !l.is_empty()) {
        let _ = docker(&vec!["network".into(), "rm".into(), id.to_string()]);
    }
    Ok(removed)
}

/// Is the daemon reachable (preflight).
pub fn live() -> bool {
    docker(&vec![
        "version".into(),
        "--format".into(),
        "{{.Server.Version}}".into(),
    ])
    .map(|o| o.status.success())
    .unwrap_or(false)
}
