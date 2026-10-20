//! CLI boundary for M1 Terrain DNS observation record, inspect, and recover.
use duskweave::m1_terrain::TerrainReceipt;
use duskweave::m1_terrain::verify_mission_permission;
use duskweave::m1_terrain_input::{TerrainDnsRequest, read_terrain_dns_file};
use duskweave::mission::OperationId;
use duskweave::postgres_m1_terrain::{PgTerrainStore, qualify_terrain};
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::postgres_terrain_history::qualify_terrain_trajectory;
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration;
use duskweave::trajectory::HistoryStatus;
use duskweave::{Fail, Res};
use postgres::Client;
use std::path::Path;
use uuid::Uuid;

fn parse_op_flag(command: &str, raw_op: &str) -> Res<Option<OperationId>> {
    if raw_op == "new" {
        return (command == "m1-terrain-record")
            .then_some(None)
            .ok_or(Fail::Input("invalid_args"));
    }
    let id = Uuid::parse_str(raw_op).map_err(|_| Fail::Input("invalid_args"))?;
    (!id.is_nil())
        .then_some(Some(OperationId(id)))
        .ok_or(Fail::Input("invalid_args"))
}

fn parse_args<'a>(command: &str, args: &'a [String]) -> Res<(&'a str, Option<OperationId>)> {
    let parsed = super::flags(args, &["input", "operation"])?;
    let input = super::flag(&parsed, "input")?;
    let raw_op = super::flag(&parsed, "operation")?;
    Ok((input, parse_op_flag(command, raw_op)?))
}

pub(super) fn terrain_connection() -> Res<Client> {
    super::database_wait::connect_owner(
        super::database_config::owner_config(
            "DW_TERRAIN_DATABASE_URL",
            "DW_TERRAIN_DATABASE_URL_FILE",
        )?,
        qualify_terrain,
    )
}

pub(super) fn trajectory_connection() -> Res<Client> {
    super::database_wait::connect_owner(
        super::database_config::owner_config(
            "DW_TRAJECTORY_DATABASE_URL",
            "DW_TRAJECTORY_DATABASE_URL_FILE",
        )?,
        qualify_terrain_trajectory,
    )
}

fn connect_qualified() -> Res<(Client, Client, Client)> {
    let rt_cfg = super::database_config::runtime_config()?;
    let tr_cfg = super::database_config::owner_config(
        "DW_TERRAIN_DATABASE_URL",
        "DW_TERRAIN_DATABASE_URL_FILE",
    )?;
    let tj_cfg = super::database_config::owner_config(
        "DW_TRAJECTORY_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL_FILE",
    )?;
    let valid = [
        rt_cfg.get_dbname().is_some_and(|d| d != "postgres"),
        rt_cfg.get_dbname() == tr_cfg.get_dbname(),
        rt_cfg.get_dbname() == tj_cfg.get_dbname(),
        rt_cfg.get_user() != tr_cfg.get_user(),
        rt_cfg.get_user() != tj_cfg.get_user(),
        tr_cfg.get_user() != tj_cfg.get_user(),
    ];
    if !valid.into_iter().all(|b| b) {
        return Err(Fail::Config("invalid_configuration"));
    }
    Ok((
        super::connect()?,
        terrain_connection()?,
        trajectory_connection()?,
    ))
}

fn map_config_fail(code: &'static str) -> (&'static str, &'static str, Fail) {
    let unq = [
        code == "unqualified_terrain_role",
        code == "unqualified_trajectory_role",
        code == "unqualified_runtime_role",
    ];
    if unq.into_iter().any(|b| b) {
        (
            "unavailable",
            "unqualified_role",
            Fail::Config("unqualified_role"),
        )
    } else {
        (
            "unavailable",
            "invalid_configuration",
            Fail::Config("invalid_configuration"),
        )
    }
}

fn map_fail(f: Fail) -> (&'static str, &'static str, Fail) {
    match f {
        Fail::Input("invalid_args") => ("unavailable", "invalid_args", Fail::Input("invalid_args")),
        Fail::Input(_) => ("unavailable", "invalid_input", Fail::Input("invalid_input")),
        Fail::Conflict(c) | Fail::State(c) => ("refused", c, Fail::State(c)),
        Fail::Config(c) => map_config_fail(c),
        Fail::Store("contract_decode") => (
            "unavailable",
            "contract_decode",
            Fail::Store("contract_decode"),
        ),
        _ => (
            "unavailable",
            "store_unavailable",
            Fail::Store("store_unavailable"),
        ),
    }
}

fn prepare_record_op(
    mission: &mut PgMissionStore,
    req: &TerrainDnsRequest,
    op_opt: Option<OperationId>,
) -> Res<Result<OperationId, &'static str>> {
    if let Some(op) = op_opt {
        return Ok(Ok(op));
    }
    if let Err(reason) = verify_mission_permission(mission, req)? {
        return Ok(Err(reason));
    }
    let op = registration::prepare_operation(&mut PgAllocator::new(super::connect()?))?;
    Ok(Ok(op))
}

fn hist_code(res: Res<HistoryStatus>) -> &'static str {
    match res {
        Ok(HistoryStatus::Completed) => "completed",
        Ok(HistoryStatus::Anomaly) => "anomaly",
        _ => "pending",
    }
}

fn run_record(
    rt: Client,
    tr: Client,
    tj: Client,
    req: &TerrainDnsRequest,
    op_opt: Option<OperationId>,
) -> Res<(TerrainReceipt, Option<Fail>)> {
    let mut mission = PgMissionStore::new(rt);
    let op = match prepare_record_op(&mut mission, req, op_opt)? {
        Ok(id) => id,
        Err(reason) => {
            return Ok((
                TerrainReceipt::failure("refused", reason, Some(req), None),
                Some(Fail::State(reason)),
            ));
        }
    };
    let mut store = PgTerrainStore::new(tr);
    let mut traj = PgTrajectory::new(tj);
    let res = store.record_dns(&mut mission, req, op);
    match res {
        Ok(Ok((ev, cur, now, auth_ok))) => {
            let hist = hist_code(traj.record_terrain(&ev));
            Ok((
                TerrainReceipt::from_event(&ev, &cur, hist, now, auth_ok),
                None,
            ))
        }
        Ok(Err(reason)) => Ok((
            TerrainReceipt::failure("refused", reason, Some(req), Some(op)),
            Some(Fail::State(reason)),
        )),
        Err(f) => {
            let (res, code, mapped) = if f == Fail::Unresolved("terrain_commit_unknown") {
                ("unresolved", "terrain_commit_unknown", f)
            } else {
                map_fail(f)
            };
            Ok((
                TerrainReceipt::failure(res, code, Some(req), Some(op)),
                Some(mapped),
            ))
        }
    }
}

fn run_inspect_or_recover(
    command: &str,
    rt: Client,
    tr: Client,
    tj: Client,
    req: &TerrainDnsRequest,
    op: OperationId,
) -> Res<(TerrainReceipt, Option<Fail>)> {
    let mut mission = PgMissionStore::new(rt);
    let mut store = PgTerrainStore::new(tr);
    let mut traj = PgTrajectory::new(tj);
    let Some((ev, cur, now, auth_ok)) = store.inspect_dns(&mut mission, req, op)? else {
        return Ok((
            TerrainReceipt::failure("unavailable", "operation_not_found", Some(req), Some(op)),
            Some(Fail::State("operation_not_found")),
        ));
    };
    let hist = if command == "m1-terrain-recover" {
        hist_code(traj.record_terrain(&ev))
    } else {
        traj.inspect_terrain(&ev)?
    };
    Ok((
        TerrainReceipt::from_event(&ev, &cur, hist, now, auth_ok),
        None,
    ))
}

pub(super) fn command(command: &str, args: &[String]) -> Res<()> {
    let mut req_captured = None;
    let mut op_captured = None;
    let outcome = (|| {
        let (path, op_opt) = parse_args(command, args)?;
        op_captured = op_opt;
        let req =
            read_terrain_dns_file(Path::new(path)).map_err(|_| Fail::Input("invalid_input"))?;
        req_captured = Some(req);
        let req = req_captured.as_ref().ok_or(Fail::Input("invalid_input"))?;
        let (rt, tr, tj) = connect_qualified()?;
        match (command, op_opt) {
            ("m1-terrain-record", _) => run_record(rt, tr, tj, req, op_opt),
            (_, Some(op)) => run_inspect_or_recover(command, rt, tr, tj, req, op),
            _ => Err(Fail::Input("invalid_args")),
        }
    })();
    let (receipt, err_opt) = match outcome {
        Ok(pair) => pair,
        Err(f) => {
            let (res, code, mapped) = map_fail(f);
            let mut receipt =
                TerrainReceipt::failure(res, code, req_captured.as_ref(), op_captured);
            if command != "m1-terrain-record" && op_captured.is_some() {
                // A failed read establishes neither rollback nor absence of history.
                receipt.publication = "unknown".to_string();
                receipt.history = "unknown".to_string();
            }
            (receipt, Some(mapped))
        }
    };
    let json = serde_json::to_string(&receipt).map_err(|_| Fail::State("receipt_encode"))?;
    println!("{json}");
    match err_opt {
        None => Ok(()),
        Some(f) => Err(f),
    }
}
