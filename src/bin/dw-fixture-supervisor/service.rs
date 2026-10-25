//! Fixed local Docker/PostgreSQL service qualification.
//!
//! Reads the container inspect document privately, decodes absent or null
//! port metadata without panicking, and refuses anything that is not the
//! pinned image running PostgreSQL on the loopback binding. Inspect JSON,
//! container environment values and raw SQL errors are never echoed.

use serde_json::{Value, json};
use std::process::Command;
use std::time::Duration;

pub const IMAGE: &str =
    "postgres@sha256:d74eeac9a635390a49bc21bd49fccd973de707e2a53a76ac49b552b8712ec46f";
const REFUSED: &str = "native_service_unqualified";
const USAGE: &str = "native_supervisor_usage_refused";
// Both probes stay well inside the bridge's 60 s helper-invocation bound.
const PROBE_WINDOW: Duration = Duration::from_secs(25);

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

/// Both fixed probes use the action owner's report-free bounded capture.
fn bounded_output(argv: &[&str]) -> Option<Vec<u8>> {
    let mut command = Command::new(argv.first()?);
    command.args(&argv[1..]);
    let captured = crate::action::capture_process(command, "", PROBE_WINDOW).ok()?;
    if captured.status != "completed" {
        return None;
    }
    captured.stdout.map(|stream| stream.bytes)
}

#[cfg(test)]
mod tests;
