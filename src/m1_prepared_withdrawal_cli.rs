//! Prepared-only withdrawal ingress; no host STOP or execution claim.
use duskweave::m1_prepared_withdrawal::{
    self, PreparedWithdrawalRecord, PreparedWithdrawalRequest,
};
use duskweave::m1_session_input::read_prepared_withdrawal_file;
use duskweave::mission::OperationId;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_prepared_withdrawal::PgPreparedWithdrawalStore;
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::withdrawal::WithdrawalHistoryPort;
use duskweave::{Fail, Res};
use std::path::Path;

fn receipt(record: &PreparedWithdrawalRecord, recover: bool) -> serde_json::Value {
    let history = super::connect().and_then(|client| {
        let mut consumer = PgTrajectory::new(client);
        if recover {
            consumer.inspect(&record.event)
        } else {
            consumer.publish(&record.event)
        }
    });
    super::durable_receipt(record.event.clone(), history)
}

/// Parse the closed command/input surface before constructing any database port.
fn parse(args: &[String]) -> Res<(OperationId, bool, PreparedWithdrawalRequest)> {
    let flags = super::flags(args, &["operation", "input", "recover"])?;
    let op = OperationId::parse(super::flag(&flags, "operation")?)
        .ok_or(Fail::Input("invalid_operation"))?;
    let recover = super::flag(&flags, "recover")?
        .parse::<bool>()
        .map_err(|_| Fail::Input("invalid_args"))?;
    let request = super::flag(&flags, "input")
        .and_then(|path| read_prepared_withdrawal_file(Path::new(path)))?;
    Ok((op, recover, request))
}

pub(super) fn command(args: &[String]) -> Res<()> {
    let (op, recover, request) = parse(args)?;
    request.validate(op)?;
    let outcome = (|| {
        let mut store = PgPreparedWithdrawalStore::new(super::connect()?);
        let mut reader = (!recover)
            .then(|| super::connect().map(PgMissionStore::new))
            .transpose()?;
        m1_prepared_withdrawal::withdraw_prepared(
            &mut store,
            reader.as_mut(),
            &request,
            op,
            recover,
        )
    })();
    let mut value = match &outcome {
        Ok(Some(record)) => receipt(record, recover),
        Ok(None) => serde_json::json!({"result":"not_committed","action":"reconcile_authority"}),
        Err(Fail::Unresolved(reason)) => serde_json::json!({
            "result":"unknown","reason":reason,"action":"recover_before_retry"}),
        Err(error) => super::failure_receipt(error, Some(op)),
    };
    value["operation"] = serde_json::json!(op);
    value["session_operation_id"] = serde_json::json!(request.session_operation_id);
    value["expected_session_generation"] = request.expected_session_generation.into();
    value["current_permission"] = false.into();
    value["dispatch_granted"] = false.into();
    value["acquisition_qualified"] = false.into();
    value["host_control_qualified"] = false.into();
    value["continuation_blocked"] = true.into();
    // Bounded receipt contains safe identities and the existing validated Mission contract.
    println!("{value}");
    outcome.map(|_| ())
}
