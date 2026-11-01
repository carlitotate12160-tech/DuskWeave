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
        if !recover {
            let _ = super::m1_run_control::signal_campaign(q.engagement_id.0, q.campaign_id.0);
        }
        // This read happens only after the local-stop port has returned.
        let client = match super::connect() {
            Ok(client) => client,
            // Indeterminate lookup failures cannot prove rejection; the live
            // lane classifies them as unknown rather than a refused write.
            Err(error) => {
                self.live = true;
                return Err(error);
            }
        };
        let mut store = PgMissionStore::new(client);
        self.live = match live_registration(&mut store, q) {
            Ok(live) => live,
            Err(error) => {
                self.live = !matches!(error, Fail::Input(_) | Fail::State(_) | Fail::Conflict(_));
                return Err(error);
            }
        };
        if self.live {
            return live_write(q, operation, recover);
        }
        let mut allocator = PgAllocator::new(super::connect()?);
        WithdrawalStore::withdraw(&mut store, q, operation, recover, &mut allocator)
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
