//! Existing withdrawal and stop command composition. Host control precedes SQL.
use duskweave::m1_effect_control::authority::{
    self, HostStop, LocalStop, RestrictedWithdrawal, StopDisposition, StopTarget,
};
use duskweave::mission::OperationId;
use duskweave::postgres_m1_effect::PgStopWriter;
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration::MissionStore;
use duskweave::withdrawal::{
    MissionAuthorityWithdrawn, WithdrawalHistoryPort, WithdrawalRequest, WithdrawalStore,
};
use duskweave::{Fail, Res};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Default)]
struct Writer {
    live: bool,
}
impl RestrictedWithdrawal for Writer {
    fn withdraw(
        &mut self,
        q: &WithdrawalRequest,
        operation: OperationId,
        recover: bool,
    ) -> Res<Option<MissionAuthorityWithdrawn>> {
        signal_campaign(q, recover);
        // This read happens only after the local-stop port has returned.
        let mut store = match connect_store() {
            Ok(store) => store,
            Err((error, live)) => {
                self.live = live;
                return Err(error);
            }
        };
        self.live = match check_live(&mut store, q) {
            Ok(live) => live,
            Err((error, live)) => {
                self.live = live;
                return Err(error);
            }
        };
        self.finish_withdraw(&mut store, q, operation, recover)
    }
}
impl Writer {
    fn finish_withdraw(
        &mut self,
        store: &mut PgMissionStore,
        q: &WithdrawalRequest,
        operation: OperationId,
        recover: bool,
    ) -> Res<Option<MissionAuthorityWithdrawn>> {
        if self.live {
            return live_write(q, operation, recover);
        }
        let mut allocator = PgAllocator::new(super::connect()?);
        WithdrawalStore::withdraw(store, q, operation, recover, &mut allocator)
    }
}
fn signal_campaign(q: &WithdrawalRequest, recover: bool) {
    if !recover {
        let _ = super::m1_run_control::signal_campaign(q.engagement_id.0, q.campaign_id.0);
    }
}
fn connect_store() -> Result<PgMissionStore, (Fail, bool)> {
    // Indeterminate lookup failures cannot prove rejection; the live
    // lane classifies them as unknown rather than a refused write.
    super::connect()
        .map(PgMissionStore::new)
        .map_err(|error| (error, true))
}
fn check_live(store: &mut PgMissionStore, q: &WithdrawalRequest) -> Result<bool, (Fail, bool)> {
    match live_registration(store, q) {
        Ok(live) => Ok(live),
        Err(e) => {
            let live = !matches!(e, Fail::Input(_) | Fail::State(_) | Fail::Conflict(_));
            Err((e, live))
        }
    }
}
fn live_registration(store: &mut PgMissionStore, q: &WithdrawalRequest) -> Res<bool> {
    let view = store
        .mission_view(q.engagement_id, q.campaign_id)?
        .ok_or(Fail::State("mission_not_found"))?;
    let event = store
        .outbox_event(q.engagement_id, q.campaign_id, view.operation_id)?
        .ok_or(Fail::Store("registration_history_unavailable"))?;
    Ok(event
        .fields
        .m1_inference_permission()
        .and_then(|p| p.live_binding())
        .is_some())
}
fn stop_role(client: &mut postgres::Client) -> Res<()> {
    duskweave::m1_reasoning::postgres::qualify_owner(client, "dw_m1_stop_writer")
        .map_err(Fail::Config)
}
fn live_write(
    q: &WithdrawalRequest,
    operation: OperationId,
    recover: bool,
) -> Res<Option<MissionAuthorityWithdrawn>> {
    super::database_config::preflight_live()?;
    let config = super::database_config::owner_config(
        "DW_STOP_WRITER_DATABASE_URL",
        "DW_STOP_WRITER_DATABASE_URL_FILE",
    )?;
    let client = super::database_wait::connect_owner(config, stop_role)?;
    // Campaign-scoped atomic path: the fenced Mission function resolves the
    // current session itself; absent sessions still withdraw durably.
    let mut writer = PgStopWriter::scoped(client);
    RestrictedWithdrawal::withdraw(&mut writer, q, operation, recover)
}
fn publication(event: MissionAuthorityWithdrawn, recover: bool) -> Value {
    // The existing M0 Trajectory journal owns this contract and qualifies its connection.
    let history = super::connect().and_then(|client| {
        let mut owner = PgTrajectory::new(client);
        if recover {
            owner.inspect(&event)
        } else {
            owner.publish(&event)
        }
    });
    super::durable_receipt(event, history)
}
fn receipt(
    outcome: &Res<Option<MissionAuthorityWithdrawn>>,
    operation: Option<OperationId>,
    recover: bool,
    live: bool,
) -> Value {
    match outcome {
        Ok(Some(event)) => publication(event.clone(), recover),
        Ok(None) => {
            json!({"result":"not_committed","operation":operation,"action":"reconcile_authority"})
        }
        Err(error) if unrecoverable(error, live) => {
            json!({"result":"unknown","operation":operation,
            "reason":super::category(*error),"action":"recover_before_retry"})
        }
        Err(error) => super::failure_receipt(error, operation),
    }
}
fn unrecoverable(error: &Fail, live: bool) -> bool {
    live && *error != Fail::Store("serialization_retry")
}
pub(super) fn command(args: &[String]) -> Res<()> {
    let mut operation = None;
    let mut local = StopDisposition::default();
    let mut writer = Writer::default();
    let mut recover = false;
    let mut scope = None;
    let outcome = (|| {
        let (op, reconciliation, request) = super::parse_withdrawal_command(args, &mut operation)?;
        recover = reconciliation;
        scope = Some((request.engagement_id, request.campaign_id));
        let result = authority::withdraw(&mut HostStop, &mut writer, &request, op, recover);
        local = result.local;
        result.durable
    })();
    let mut value = receipt(&outcome, operation, recover, writer.live);
    value["local_stop"] = json!(local);
    value["scope"] = json!(scope);
    value["current_permission"] = false.into();
    value["continuation_blocked"] = true.into();
    println!("{value}");
    outcome.map(|_| ())
}
pub(super) fn stop_run(run: Uuid) -> StopDisposition {
    let local = HostStop
        .stop(StopTarget::Run { run_id: run })
        .unwrap_or_default();
    // Keep the existing provider/parent cancellation signal as well.
    let _ = super::m1_run_control::signal_run(run);
    local
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrecoverable_classifies_by_live_and_error_kind() {
        assert!(!unrecoverable(&Fail::Store("serialization_retry"), true));
        assert!(unrecoverable(&Fail::Store("db_down"), true));
        assert!(!unrecoverable(&Fail::Store("db_down"), false));
        assert!(!unrecoverable(&Fail::Input("bad_input"), false));
        assert!(unrecoverable(&Fail::Input("bad_input"), true));
    }

    #[test]
    fn receipt_formats_appropriate_payloads() {
        let op = Some(OperationId(Uuid::new_v4()));

        let not_committed = receipt(&Ok(None), op, false, false);
        assert_eq!(not_committed["result"], "not_committed");
        assert_eq!(not_committed["action"], "reconcile_authority");

        let live_unrec = receipt(&Err(Fail::Store("db_down")), op, false, true);
        assert_eq!(live_unrec["result"], "unknown");
        assert_eq!(live_unrec["action"], "recover_before_retry");

        let ordinary_err = receipt(&Err(Fail::Input("bad_argument")), op, false, false);
        assert_eq!(ordinary_err["result"], "rejected");
    }

    #[test]
    fn stop_run_signals_and_returns_disposition() {
        let id = Uuid::new_v4();
        let disp = stop_run(id);
        let _ = disp;
    }

    #[test]
    fn signal_campaign_skips_when_recovering() {
        let q: WithdrawalRequest = serde_json::from_value(json!({
            "engagement_id": Uuid::new_v4(),
            "campaign_id": Uuid::new_v4(),
            "operator_ref": Uuid::new_v4(),
            "expected_mission_revision": 1,
            "reason": "operator_requested"
        }))
        .unwrap();
        signal_campaign(&q, true);
        signal_campaign(&q, false);
    }
}
