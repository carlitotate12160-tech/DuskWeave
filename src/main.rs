//! duskweave CLI: wiring only. Reads env config, connects to the local
//! loopback PostgreSQL, composes the M0A use cases and prints bounded
//! receipts (safe identifiers and categories only).
#![forbid(unsafe_code)]

use duskweave::input::read_register_file;
use duskweave::mission::{CampaignId, EngagementId, OperationId, RegistrationInput};
use duskweave::postgres_mission::{PgAllocator, PgMissionStore, qualify_runtime};
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration;
use duskweave::trajectory::HistoryStatus;
use duskweave::{Fail, Res};
use postgres::NoTls;
use std::env;
use std::path::Path;
use std::process::ExitCode;

mod authority_confirmation;
mod planning_cli;

fn category(f: Fail) -> &'static str {
    match f {
        Fail::Input(c) | Fail::Conflict(c) | Fail::State(c) => c,
        Fail::Unresolved(c) | Fail::Store(c) | Fail::Config(c) => c,
    }
}

fn flags<'a>(args: &'a [String], allowed: &[&str]) -> Res<Vec<(&'a str, &'a str)>> {
    let mut out: Vec<(&'a str, &'a str)> = Vec::new();
    let mut i = 2;
    while i < args.len() {
        let name = args[i]
            .strip_prefix("--")
            .ok_or(Fail::Input("invalid_args"))?;
        if !allowed.contains(&name) || out.iter().any(|(n, _)| *n == name) {
            return Err(Fail::Input("invalid_args"));
        }
        let value = args
            .get(i + 1)
            .filter(|v| !v.starts_with("--"))
            .ok_or(Fail::Input("invalid_args"))?;
        out.push((name, value));
        i += 2;
    }
    if allowed.iter().all(|req| out.iter().any(|(n, _)| n == req)) {
        Ok(out)
    } else {
        Err(Fail::Input("invalid_args"))
    }
}

fn flag<'a>(args: &[(&'a str, &'a str)], name: &str) -> Res<&'a str> {
    args.iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| *v)
        .ok_or(Fail::Input("invalid_args"))
}

fn runtime_config() -> Res<postgres::Config> {
    let raw = env::var("DW_DATABASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .ok_or(Fail::Config("missing_env"))?;
    let cfg: postgres::Config = raw.parse().map_err(|_| Fail::Config("invalid_dsn"))?;
    check_config(&cfg)?;
    Ok(cfg)
}

fn check_config(cfg: &postgres::Config) -> Res<()> {
    let loopback = matches!(
        cfg.get_hosts(),
        [postgres::config::Host::Tcp(h)] if h == "127.0.0.1" || h == "::1"
    );
    if !loopback {
        return Err(Fail::Config("non_loopback_host"));
    }
    if cfg.get_user().is_none_or(str::is_empty) || cfg.get_password().is_none_or(|p| p.is_empty()) {
        return Err(Fail::Config("missing_credentials"));
    }
    Ok(())
}

fn connect() -> Res<postgres::Client> {
    let mut client = runtime_config()?
        .connect(NoTls)
        .map_err(|_| Fail::Config("connect_failed"))?;
    qualify_runtime(&mut client)?;
    Ok(client)
}

fn status_str(s: HistoryStatus) -> &'static str {
    match s {
        HistoryStatus::Completed => "completed",
        HistoryStatus::Pending => "pending",
        HistoryStatus::Anomaly => "anomaly",
    }
}

fn scope(f: &[(&str, &str)]) -> Res<(EngagementId, CampaignId)> {
    let e = EngagementId::parse(flag(f, "engagement")?).ok_or(Fail::Input("invalid_args"))?;
    let c = CampaignId::parse(flag(f, "campaign")?).ok_or(Fail::Input("invalid_args"))?;
    Ok((e, c))
}

fn cmd_prepare(args: &[String]) -> Res<()> {
    let f = flags(args, &["engagement", "campaign"])?;
    let (e, _c) = scope(&f)?;
    let mut alloc = PgAllocator::new(connect()?);
    let op = registration::prepare_operation(&mut alloc)?;
    // codeql[rust/cleartext-logging] scoped UUID handles only, no secret data
    println!("operation={op} engagement={e}");
    Ok(())
}

fn emit_register(got: Res<registration::Receipt>, op: OperationId) -> Res<()> {
    match got {
        // codeql[rust/cleartext-logging] bounded receipt: safe IDs/status only
        Ok(r) => println!(
            "register result=accepted engagement={} campaign={} operation={} event={} history={}",
            r.engagement_id,
            r.campaign_id,
            r.operation_id,
            r.event_id,
            status_str(r.history)
        ),
        Err(Fail::Store("commit_unknown")) => {
            println!("register result=unknown operation={op} action=reconcile_before_retry")
        }
        Err(f) => return Err(f),
    }
    Ok(())
}

fn register_ports() -> Res<(PgAllocator, PgMissionStore, PgTrajectory)> {
    Ok((
        PgAllocator::new(connect()?),
        PgMissionStore::new(connect()?),
        PgTrajectory::new(connect()?),
    ))
}

fn cmd_register(args: &[String]) -> Res<()> {
    let f = flags(args, &["operation", "input"])?;
    let op = OperationId::parse(flag(&f, "operation")?).ok_or(Fail::Input("invalid_args"))?;
    let input: RegistrationInput = read_register_file(Path::new(flag(&f, "input")?))?;
    let (mut alloc, mut store, mut traj) = register_ports()?;
    emit_register(
        registration::register(&mut alloc, &mut store, &mut traj, op, &input),
        op,
    )
}

fn emit_inspect(v: Option<registration::InspectView>) {
    match v {
        Some(v) => println!(
            "inspect mission.revision={} mission.operation={} mission.mode={:?} history.event={} history.status={}",
            v.mission.revision,
            v.mission.operation_id,
            v.mission.exercise_mode,
            v.event_id
                .map(|i| i.to_string())
                .unwrap_or_else(|| "none".to_string()),
            status_str(v.history)
        ),
        None => println!("inspect result=empty"),
    }
}

fn cmd_inspect(args: &[String]) -> Res<()> {
    let f = flags(args, &["engagement", "campaign"])?;
    let (e, c) = scope(&f)?;
    let mut store = PgMissionStore::new(connect()?);
    let mut traj = PgTrajectory::new(connect()?);
    emit_inspect(registration::inspect(&mut store, &mut traj, e, c)?);
    Ok(())
}

fn cmd_reconcile(args: &[String]) -> Res<()> {
    let f = flags(args, &["engagement", "campaign", "operation"])?;
    let (e, c) = scope(&f)?;
    let op = OperationId::parse(flag(&f, "operation")?).ok_or(Fail::Input("invalid_args"))?;
    let (mut _a, mut store, mut traj) = register_ports()?;
    let r = registration::reconcile(&mut store, &mut traj, e, c, op)?;
    println!("reconcile result={r:?}");
    Ok(())
}

fn run(args: &[String]) -> Res<()> {
    match args
        .get(1)
        .map(String::as_str)
        .ok_or(Fail::Input("missing_command"))?
    {
        "prepare-operation" => cmd_prepare(args),
        "register" => cmd_register(args),
        "assess" => planning_cli::cmd_assess(args),
        "planning-history" => planning_cli::cmd_planning_history(args),
        "inspect" => cmd_inspect(args),
        "reconcile" => cmd_reconcile(args),
        _ => Err(Fail::Input("unknown_command")),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(f) => {
            println!("error={}", category(f));
            ExitCode::FAILURE
        }
    }
}
