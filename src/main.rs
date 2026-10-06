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
use duskweave::withdrawal::{
    MissionAuthorityWithdrawn, OBLIGATION, WithdrawalHistoryPort, WithdrawalRequest,
    WithdrawalStore,
};
use duskweave::withdrawal_input::read_withdrawal_file;
use duskweave::{Fail, Res};
use postgres::NoTls;
use std::env;
use std::path::Path;
use std::process::ExitCode;

mod authority_confirmation;
mod m1_policy_cli;
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
    match &v {
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
    if let Some((summary, history)) = v.and_then(|v| v.m1_permission.map(|p| (p, v.history))) {
        emit_permission(&summary, history);
    }
}

fn emit_permission(p: &duskweave::m1_permission::PermissionSummary, history: HistoryStatus) {
    let limits = &p.campaign_limits;
    println!(
        "m1 historical_configuration policy_version={} operator={} authority={} authority_revision={} goal={} vantage={} discovery_rules={} contact_rules={} excluded_names={} starts_at={} ends_at={} episodes={} provider_calls={} dns_questions={} dns_followups={} tcp_connections={} head_requests={} history={} current_permission=false acquisition_qualified=false",
        p.policy_version,
        p.operator_ref,
        p.authority_ref,
        p.authority_revision,
        p.goal_ref,
        p.vantage_ref,
        p.discovery_rules,
        p.contact_rules,
        p.excluded_names,
        p.starts_at,
        p.ends_at,
        limits.episodes,
        limits.provider_calls,
        limits.dns_questions,
        limits.dns_followups,
        limits.tcp_connections,
        limits.head_requests,
        status_str(history)
    );
}

fn cmd_inspect(args: &[String]) -> Res<()> {
    let f = flags(args, &["engagement", "campaign"])?;
    let (e, c) = scope(&f)?;
    let mut store = PgMissionStore::new(connect()?);
    let mut traj = PgTrajectory::new(connect()?);
    emit_inspect(registration::inspect(&mut store, &mut traj, e, c)?);
    Ok(())
}

/// Flag/input parsing in the fixed order; the parsed operation is captured
/// even when a later step fails so the bounded receipt still names the target.
fn parse_withdrawal_command(
    args: &[String],
    captured: &mut Option<OperationId>,
) -> Res<(OperationId, bool, WithdrawalRequest)> {
    let parsed = flags(args, &["operation", "input", "recover"])?;
    let operation =
        OperationId::parse(flag(&parsed, "operation")?).ok_or(Fail::Input("invalid_args"))?;
    *captured = Some(operation);
    let recover = flag(&parsed, "recover")?
        .parse::<bool>()
        .map_err(|_| Fail::Input("invalid_args"))?;
    let request = flag(&parsed, "input").and_then(|path| read_withdrawal_file(Path::new(path)))?;
    Ok((operation, recover, request))
}

fn durable_receipt(
    event: MissionAuthorityWithdrawn,
    history: Res<duskweave::trajectory::Delivered>,
) -> serde_json::Value {
    let view = duskweave::planning_history::history_result(history);
    serde_json::json!({
        "result": "durable", "contract": event, "authority_state": "withdrawn",
        "owner_revision": 2, "publication_obligation": OBLIGATION,
        "history_source": "trajectory", "history": view.state, "history_reason": view.reason,
        "complete_history": view.complete,
        "action": if view.complete { "none" } else if view.state == "unknown" {
            "recover_history_before_retry"
        } else { "retry_known_withdrawal" },
    })
}

fn failure_receipt(error: &Fail, operation: Option<OperationId>) -> serde_json::Value {
    serde_json::json!({
        "result": if *error == Fail::Store("commit_unknown") { "unknown" } else { "rejected" },
        "operation": operation, "reason": category(*error),
        "action": if *error == Fail::Store("commit_unknown") { "recover_before_retry" } else { "reconcile_authority" },
    })
}

fn cmd_withdraw(args: &[String]) -> Res<()> {
    let mut operation = None;
    let outcome = (|| {
        let (operation, recover, request) = parse_withdrawal_command(args, &mut operation)?;
        let mut store = PgMissionStore::new(connect()?);
        let mut allocator = PgAllocator::new(connect()?);
        let Some(event) = store.withdraw(&request, operation, recover, &mut allocator)? else {
            return Ok(
                serde_json::json!({"result": "not_committed", "operation": operation,
            "action": "reconcile_authority"}),
            );
        };
        // Publish failure stays unknown; it cannot reject a committed withdrawal.
        let history = connect().and_then(|client| {
            let mut consumer = PgTrajectory::new(client);
            if recover {
                consumer.inspect(&event)
            } else {
                consumer.publish(&event)
            }
        });
        Ok(durable_receipt(event, history))
    })();
    let mut receipt = match &outcome {
        Ok(receipt) => receipt.clone(),
        Err(error) => failure_receipt(error, operation),
    };
    receipt["current_permission"] = false.into();
    receipt["continuation_blocked"] = true.into();
    println!("{receipt}");
    outcome.map(|_| ())
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

fn run(command: &str, args: &[String]) -> Res<()> {
    match command {
        "prepare-operation" => cmd_prepare(args),
        "register" => cmd_register(args),
        "assess" => planning_cli::cmd_assess(args),
        "withdraw" => cmd_withdraw(args),
        "planning-history" => planning_cli::cmd_planning_history(args),
        "inspect" => cmd_inspect(args),
        "reconcile" => cmd_reconcile(args),
        "m1-policy-check" => m1_policy_cli::cmd_policy_check(args),
        _ => Err(Fail::Input("unknown_command")),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let outcome = args
        .get(1)
        .ok_or(Fail::Input("missing_command"))
        .and_then(|command| run(command, &args));
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(f) => {
            println!("error={}", category(f));
            ExitCode::FAILURE
        }
    }
}
