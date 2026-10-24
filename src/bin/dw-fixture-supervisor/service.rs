//! Fixed local Docker/PostgreSQL service qualification.
//!
//! Reads the container inspect document privately, decodes absent or null
//! port metadata without panicking, and refuses anything that is not the
//! pinned image running PostgreSQL on the loopback binding. Inspect JSON,
//! container environment values and raw SQL errors are never echoed.

use serde_json::{Value, json};
use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

pub const IMAGE: &str =
    "postgres@sha256:d74eeac9a635390a49bc21bd49fccd973de707e2a53a76ac49b552b8712ec46f";
const REFUSED: &str = "native_service_unqualified";
const USAGE: &str = "native_supervisor_usage_refused";
// Both probes stay well inside the bridge's 60 s helper-invocation bound.
const PROBE_WINDOW: Duration = Duration::from_secs(25);
const POLL_INTERVAL: Duration = Duration::from_millis(25);

pub struct ServiceOptions {
    pub container: String,
    pub ci: bool,
    pub version_prefix: Option<String>,
    pub version_exact: Option<String>,
}

/// Parse the closed `service-check` flag set.
pub fn parse_options(arguments: &[String]) -> Result<ServiceOptions, &'static str> {
    let mut options = ServiceOptions {
        container: String::new(),
        ci: false,
        version_prefix: None,
        version_exact: None,
    };
    let mut index = 0;
    while index < arguments.len() {
        let consumed = apply_flag(&mut options, &arguments[index..])?;
        index += consumed;
    }
    Ok(options)
}

fn apply_flag(options: &mut ServiceOptions, rest: &[String]) -> Result<usize, &'static str> {
    match rest.first().map(String::as_str) {
        Some("--ci") => {
            options.ci = true;
            Ok(1)
        }
        Some("--container") => assign_value(&mut options.container, rest),
        Some("--version-prefix") => assign_optional(&mut options.version_prefix, rest),
        Some("--version-exact") => assign_optional(&mut options.version_exact, rest),
        _ => Err(USAGE),
    }
}

fn assign_value(slot: &mut String, rest: &[String]) -> Result<usize, &'static str> {
    let value = rest.get(1).ok_or(USAGE)?;
    slot.clone_from(value);
    Ok(2)
}

fn assign_optional(slot: &mut Option<String>, rest: &[String]) -> Result<usize, &'static str> {
    let value = rest.get(1).ok_or(USAGE)?;
    *slot = Some(value.clone());
    Ok(2)
}

/// One closed service-check command invocation.
pub fn command(arguments: &[String]) -> Result<Value, &'static str> {
    command_with(arguments, docker_inspect, docker_server_version)
}

/// The same closed command over injectable docker probes, so the qualification
/// logic is testable without owning a live container.
pub fn command_with(
    arguments: &[String],
    inspect: fn(&str) -> Option<Vec<u8>>,
    server_version: fn(&str) -> Option<String>,
) -> Result<Value, &'static str> {
    let options = parse_options(arguments)?;
    let one_expectation = options.version_prefix.is_some() != options.version_exact.is_some();
    if !one_expectation || options.container.is_empty() {
        return Err(USAGE);
    }
    if !container_allowed(&options.container, options.ci) {
        return Err(REFUSED);
    }
    match qualify_with(&options, inspect, server_version) {
        Some(version) => Ok(json!({
            "ok": true,
            "container": options.container,
            "server_version": version,
        })),
        None => Err(REFUSED),
    }
}

/// Only the two owned container names are accepted; CI further requires its
/// own isolated `dwpg` service.
fn container_allowed(container: &str, ci: bool) -> bool {
    let known = matches!(container, "dwpg-r3" | "dwpg");
    known && !(ci && container != "dwpg")
}

fn qualify_with(
    options: &ServiceOptions,
    inspect: fn(&str) -> Option<Vec<u8>>,
    server_version: fn(&str) -> Option<String>,
) -> Option<String> {
    let document: Value = serde_json::from_slice(&inspect(&options.container)?).ok()?;
    if !evaluate_inspect(&document) {
        return None;
    }
    let observed = server_version(&options.container)?;
    version_matches(&observed, options).then_some(observed)
}

fn docker_inspect(container: &str) -> Option<Vec<u8>> {
    bounded_output(&["docker", "inspect", container])
}

fn docker_server_version(container: &str) -> Option<String> {
    let output = bounded_output(&[
        "docker",
        "exec",
        "-i",
        container,
        "psql",
        "-X",
        "-U",
        "postgres",
        "-d",
        "postgres",
        "-At",
        "-c",
        "SHOW server_version",
    ])?;
    Some(String::from_utf8_lossy(&output).trim().to_string())
}

/// The inspect predicate; null or absent port metadata fails closed.
pub fn evaluate_inspect(document: &Value) -> bool {
    let Some(entry) = document.as_array().and_then(|entries| entries.first()) else {
        return false;
    };
    let image_ok = entry.pointer("/Config/Image").and_then(Value::as_str) == Some(IMAGE);
    let running = entry.pointer("/State/Running").and_then(Value::as_bool) == Some(true);
    image_ok && running && loopback_bound(entry)
}

fn loopback_bound(entry: &Value) -> bool {
    let bindings = entry
        .get("NetworkSettings")
        .and_then(|settings| settings.get("Ports"))
        .and_then(|ports| ports.get("5432/tcp"));
    let Some(list) = bindings.and_then(Value::as_array) else {
        return false;
    };
    list.iter().any(|binding| {
        binding.get("HostIp").and_then(Value::as_str) == Some("127.0.0.1")
            && binding.get("HostPort").and_then(Value::as_str) == Some("5432")
    })
}

fn version_matches(observed: &str, options: &ServiceOptions) -> bool {
    let token = observed.split_whitespace().next().unwrap_or("");
    if let Some(exact) = options.version_exact.as_deref() {
        return token == exact;
    }
    options
        .version_prefix
        .as_deref()
        .is_some_and(|prefix| token.starts_with(prefix))
}

/// Run one helper command with both pipes drained and a hard time bound.
fn bounded_output(argv: &[&str]) -> Option<Vec<u8>> {
    let mut child = spawn_probe(argv)?;
    let stdout = child.stdout.take()?;
    let stderr = child.stderr.take()?;
    let collector = collect(stdout);
    let _drained = collect(stderr);
    let status = wait_bounded(&mut child)?;
    let output = collector.join().ok()?;
    status.success().then_some(output)
}

fn collect(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        bytes
    })
}

fn spawn_probe(argv: &[&str]) -> Option<Child> {
    Command::new(argv.first()?)
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()
}

fn wait_bounded(child: &mut Child) -> Option<ExitStatus> {
    let deadline = Instant::now() + PROBE_WINDOW;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(_) => return None,
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inspect(ports: Value) -> Value {
        serde_json::json!([{
            "Config": { "Image": IMAGE },
            "State": { "Running": true },
            "NetworkSettings": { "Ports": ports },
        }])
    }

    #[test]
    fn qualified_loopback_binding_is_accepted() {
        let ports = json!({ "5432/tcp": [ { "HostIp": "127.0.0.1", "HostPort": "5432" } ] });
        assert!(evaluate_inspect(&inspect(ports)));
    }

    #[test]
    fn null_or_absent_port_metadata_is_refused_without_panic() {
        let variants = [
            Value::Null,
            json!({}),
            json!({ "5432/tcp": null }),
            json!({ "5432/tcp": [] }),
        ];
        for ports in variants {
            assert!(!evaluate_inspect(&inspect(ports.clone())), "{ports}");
        }
        assert!(!evaluate_inspect(&json!(null)));
        assert!(!evaluate_inspect(&json!([{}])));
    }

    #[test]
    fn foreign_binding_or_state_or_image_is_refused() {
        let remote = json!({ "5432/tcp": [ { "HostIp": "0.0.0.0", "HostPort": "5432" } ] });
        assert!(!evaluate_inspect(&inspect(remote)));
        let other_port = json!({ "5432/tcp": [ { "HostIp": "127.0.0.1", "HostPort": "5433" } ] });
        assert!(!evaluate_inspect(&inspect(other_port)));
        let mut stopped =
            inspect(json!({ "5432/tcp": [ { "HostIp": "127.0.0.1", "HostPort": "5432" } ] }));
        stopped[0]["State"]["Running"] = json!(false);
        assert!(!evaluate_inspect(&stopped));
        let mut wrong_image =
            inspect(json!({ "5432/tcp": [ { "HostIp": "127.0.0.1", "HostPort": "5432" } ] }));
        wrong_image[0]["Config"]["Image"] = json!("postgres:latest");
        assert!(!evaluate_inspect(&wrong_image));
    }

    #[test]
    fn version_expectations_are_closed() {
        let options = |prefix: Option<&str>, exact: Option<&str>| ServiceOptions {
            container: "dwpg-r3".into(),
            ci: false,
            version_prefix: prefix.map(str::to_string),
            version_exact: exact.map(str::to_string),
        };
        assert!(version_matches("17.11", &options(Some("17."), None)));
        assert!(!version_matches("16.2", &options(Some("17."), None)));
        assert!(version_matches("17.11", &options(None, Some("17.11"))));
        assert!(!version_matches(
            "17.11.2 compiled",
            &options(None, Some("17.11"))
        ));
        assert!(!version_matches("", &options(Some("17."), None)));
    }

    #[test]
    fn only_owned_containers_are_allowed() {
        assert!(container_allowed("dwpg-r3", false));
        assert!(container_allowed("dwpg", false));
        assert!(container_allowed("dwpg", true));
        assert!(!container_allowed("dwpg-r3", true));
        assert!(!container_allowed("other", false));
    }

    #[test]
    fn flag_parsing_is_closed() {
        let arguments = ["--container", "dwpg-r3", "--version-prefix", "17."].map(String::from);
        let options = parse_options(&arguments).expect("options");
        assert_eq!(options.container, "dwpg-r3");
        assert!(!options.ci);
        let ci = ["--container", "dwpg", "--ci", "--version-exact", "17.11"].map(String::from);
        assert!(parse_options(&ci).is_ok());
        assert!(parse_options(&["--unknown".to_string()]).is_err());
        assert!(parse_options(&["--container".to_string()]).is_err());
    }

    fn qualified_inspect(_: &str) -> Option<Vec<u8>> {
        let ports = json!({ "5432/tcp": [ { "HostIp": "127.0.0.1", "HostPort": "5432" } ] });
        Some(
            json!([{ "Config": { "Image": IMAGE }, "State": { "Running": true },
                     "NetworkSettings": { "Ports": ports } }])
            .to_string()
            .into_bytes(),
        )
    }

    fn fixed_version(_: &str) -> Option<String> {
        Some("17.11 (Debian 17.11-1.pgdg13+2)".into())
    }

    fn absent_probe(_: &str) -> Option<Vec<u8>> {
        None
    }

    fn absent_version(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn qualified_service_reports_only_safe_metadata() {
        let arguments = ["--container", "dwpg-r3", "--version-exact", "17.11"].map(String::from);
        let value = command_with(&arguments, qualified_inspect, fixed_version).expect("qualified");
        assert_eq!(value["ok"], true);
        assert_eq!(value["container"], "dwpg-r3");
        assert_eq!(value["server_version"], "17.11 (Debian 17.11-1.pgdg13+2)");
        let prefix = ["--container", "dwpg-r3", "--version-prefix", "17."].map(String::from);
        assert!(command_with(&prefix, qualified_inspect, fixed_version).is_ok());
    }

    #[test]
    fn version_mismatch_or_unreachable_docker_is_refused() {
        let mismatch = ["--container", "dwpg-r3", "--version-exact", "18.0"].map(String::from);
        assert!(command_with(&mismatch, qualified_inspect, fixed_version).is_err());
        let unreachable = ["--container", "dwpg-r3", "--version-prefix", "17."].map(String::from);
        assert!(command_with(&unreachable, absent_probe, fixed_version).is_err());
        let no_version = ["--container", "dwpg-r3", "--version-prefix", "17."].map(String::from);
        assert!(command_with(&no_version, qualified_inspect, absent_version).is_err());
    }
}
