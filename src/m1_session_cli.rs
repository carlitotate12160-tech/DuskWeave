use duskweave::m1_session::{self, FenceOutcome};
use duskweave::m1_session_input::read_session_file;
use duskweave::mission::OperationId;
use duskweave::postgres_m1_session::PostgresSessionFence;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::{Fail, Res};
use postgres::Client;
use std::path::Path;
use uuid::Uuid;

#[derive(serde::Serialize)]
struct SessionReceipt<'a> {
    outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    record: Option<&'a m1_session::SessionRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refusal_reason: Option<&'static str>,
    current_permission: bool,
    dispatch_granted: bool,
    acquisition_qualified: bool,
    host_control_qualified: bool,
}

impl<'a> SessionReceipt<'a> {
    fn from_outcome(outcome: &'a FenceOutcome) -> Self {
        match outcome {
            FenceOutcome::Durable(record) => Self {
                outcome: "durable",
                record: Some(record),
                refusal_reason: None,
                current_permission: false,
                dispatch_granted: false,
                acquisition_qualified: false,
                host_control_qualified: false,
            },
            FenceOutcome::Missing => Self {
                outcome: "missing",
                record: None,
                refusal_reason: None,
                current_permission: false,
                dispatch_granted: false,
                acquisition_qualified: false,
                host_control_qualified: false,
            },
            FenceOutcome::Refused(reason) => Self {
                outcome: "refused",
                record: None,
                refusal_reason: Some(reason),
                current_permission: false,
                dispatch_granted: false,
                acquisition_qualified: false,
                host_control_qualified: false,
            },
            FenceOutcome::Unknown => Self {
                outcome: "unknown",
                record: None,
                refusal_reason: None,
                current_permission: false,
                dispatch_granted: false,
                acquisition_qualified: false,
                host_control_qualified: false,
            },
        }
    }
}

pub fn dispatch(command: &str, args: &[String]) -> Res<()> {
    if command != "m1-session" {
        return Err(Fail::Input("unknown_command"));
    }
    let parsed = crate::flags(args, &["action", "operation", "input"])?;
    let action = crate::flag(&parsed, "action")?;
    let operation_str = crate::flag(&parsed, "operation")?;
    let input_path = Path::new(crate::flag(&parsed, "input")?);
    run_cli(&mut crate::connect()?, action, operation_str, input_path)
}

fn run_cli(
    client: &mut Client,
    action: &str,
    operation_str: &str,
    input_path: &Path,
) -> Res<()> {
    let operation = OperationId(
        Uuid::parse_str(operation_str).map_err(|_| Fail::Input("invalid_operation_uuid"))?,
    );
    let request = read_session_file(input_path)?;

    let outcome = match action {
        "prepare" => {
            let mut store = PgMissionStore::new(crate::connect()?);
            let mut tx = client.transaction().map_err(|_| Fail::Store("tx_begin"))?;
            let mut fence = PostgresSessionFence { tx: &mut tx };
            let out = m1_session::prepare(&mut store, &mut fence, &request, operation)?;
            tx.commit().map_err(|_| Fail::Store("commit_unknown"))?;
            out
        }
        "release" => {
            let mut tx = client.transaction().map_err(|_| Fail::Store("tx_begin"))?;
            let mut fence = PostgresSessionFence { tx: &mut tx };
            let out = m1_session::release(&mut fence, &request, operation)?;
            tx.commit().map_err(|_| Fail::Store("commit_unknown"))?;
            out
        }
        "recover" => {
            let mut tx = client.transaction().map_err(|_| Fail::Store("tx_begin"))?;
            let mut fence = PostgresSessionFence { tx: &mut tx };
            let out = m1_session::recover(&mut fence, &request, operation)?;
            tx.rollback().unwrap_or(());
            out
        }
        _ => return Err(Fail::Input("invalid_args")),
    };

    let receipt = SessionReceipt::from_outcome(&outcome);
    let json = serde_json::to_string(&receipt).unwrap();
    println!("{}", json);
    Ok(())
}
