use duskweave::{Fail, Res};
use std::env;

const LIMIT: usize = 16 * 1024;

/// Parse the selected source for every admitted owner before any run write or connection.
/// Qualification and current authority remain with the actual owner connection.
pub(super) fn preflight_run(provider: bool) -> Res<()> {
    for (owner, url, file) in [
        ("core", "DW_DATABASE_URL", "DW_DATABASE_URL_FILE"),
        (
            "broker",
            "DW_INFERENCE_BROKER_DATABASE_URL",
            "DW_INFERENCE_BROKER_DATABASE_URL_FILE",
        ),
        (
            "cognition",
            "DW_COGNITION_DATABASE_URL",
            "DW_COGNITION_DATABASE_URL_FILE",
        ),
        (
            "trajectory",
            "DW_TRAJECTORY_DATABASE_URL",
            "DW_TRAJECTORY_DATABASE_URL_FILE",
        ),
        (
            "terrain",
            "DW_TERRAIN_DATABASE_URL",
            "DW_TERRAIN_DATABASE_URL_FILE",
        ),
    ] {
        if let Err(error) = owner_config(url, file) {
            println!("config_owner={owner}");
            return Err(error);
        }
    }
    #[cfg(windows)]
    if env::var_os("SystemRoot").is_none_or(|v| v.is_empty()) {
        println!("config_owner=system");
        return Err(Fail::Config("missing_env"));
    }
    if provider && env::var_os("DW_M1_RUN_CONTROL_ROOT").is_none_or(|v| v.is_empty()) {
        println!("config_owner=control");
        return Err(Fail::Config("run_control_unqualified"));
    }
    Ok(())
}

pub(super) fn runtime_config() -> Res<postgres::Config> {
    owner_config("DW_DATABASE_URL", "DW_DATABASE_URL_FILE")
}

pub(super) fn owner_config(url: &str, file: &str) -> Res<postgres::Config> {
    let mode = match env::var("DW_DATABASE_CONFIG_MODE") {
        Ok(mode) => mode,
        Err(env::VarError::NotPresent) => "file".into(),
        Err(_) => return Err(Fail::Config("invalid_database_config_mode")),
    };
    parse_config(&load_source(&mode, url, file)?)
}

fn load_source(mode: &str, url: &str, file: &str) -> Res<String> {
    match mode {
        "file" => {
            require_absent(url)?;
            file_source(file)
        }
        "env-local" => {
            require_absent(file)?;
            env::var(url)
                .ok()
                .filter(|raw| !raw.trim().is_empty())
                .ok_or(Fail::Config("missing_env"))
        }
        _ => Err(Fail::Config("invalid_database_config_mode")),
    }
}

fn require_absent(name: &str) -> Res<()> {
    if env::var_os(name).is_some() {
        return Err(Fail::Config("database_config_conflict"));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn file_source(_file: &str) -> Res<String> {
    Err(Fail::Config("database_file_platform_unqualified"))
}

#[cfg(target_os = "linux")]
fn file_source(_file: &str) -> Res<String> {
    use rustix::fs::{CWD, Mode, OFlags, ResolveFlags, openat2};
    use std::io::Read;
    let path = env::var_os(_file)
        .map(std::path::PathBuf::from)
        .ok_or(Fail::Config("missing_database_url_file"))?;
    if !path.is_absolute() {
        return Err(Fail::Config("database_file_absolute_required"));
    }
    let fd = openat2(
        CWD,
        &path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS,
    )
    .map_err(|_| Fail::Config("database_file_open_failed"))?;
    let file = std::fs::File::from(fd);
    qualify_file(&file)?;
    let mut bytes = Vec::new();
    file.take((LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Fail::Config("database_file_read_failed"))?;
    if bytes.len() > LIMIT {
        return Err(Fail::Config("database_config_oversize"));
    }
    String::from_utf8(bytes).map_err(|_| Fail::Config("database_file_invalid_utf8"))
}

#[cfg(target_os = "linux")]
fn qualify_file(file: &std::fs::File) -> Res<()> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file
        .metadata()
        .map_err(|_| Fail::Config("database_file_unsafe"))?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || (metadata.uid() != 0 && metadata.uid() != rustix::process::geteuid().as_raw())
        || !matches!(metadata.mode() & 0o7777, 0o400 | 0o600)
    {
        return Err(Fail::Config("database_file_unsafe"));
    }
    Ok(())
}

fn parse_config(raw: &str) -> Res<postgres::Config> {
    if raw.len() > LIMIT {
        return Err(Fail::Config("database_config_oversize"));
    }
    if raw.trim().is_empty() {
        return Err(Fail::Config("invalid_dsn"));
    }
    let cfg: postgres::Config = raw
        .trim()
        .parse()
        .map_err(|_| Fail::Config("invalid_dsn"))?;
    check_config(&cfg)?;
    Ok(cfg)
}

fn check_config(cfg: &postgres::Config) -> Res<()> {
    let loopback = matches!(
        cfg.get_hosts(),
        [postgres::config::Host::Tcp(h)] if h == "127.0.0.1" || h == "::1"
    );
    if !loopback || !cfg.get_hostaddrs().is_empty() {
        return Err(Fail::Config("non_loopback_host"));
    }
    if cfg.get_user().is_none_or(str::is_empty) || cfg.get_password().is_none_or(|p| p.is_empty()) {
        return Err(Fail::Config("missing_credentials"));
    }
    Ok(())
}
