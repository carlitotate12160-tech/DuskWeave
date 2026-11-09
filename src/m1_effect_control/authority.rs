//! Restrictive host actions and withdrawal ordering through typed ports.
use crate::Res;
use crate::mission::OperationId;
use crate::withdrawal::{MissionAuthorityWithdrawn, WithdrawalRequest};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum StopTarget {
    Run {
        run_id: Uuid,
    },
    Campaign {
        engagement_id: Uuid,
        campaign_id: Uuid,
    },
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StopDisposition {
    pub stop_latched: bool,
    pub disposal_confirmed: bool,
}
pub trait LocalStop {
    fn stop(&mut self, target: StopTarget) -> Res<StopDisposition>;
}
pub trait RestrictedWithdrawal {
    fn withdraw(
        &mut self,
        request: &WithdrawalRequest,
        operation: OperationId,
        recover: bool,
    ) -> Res<Option<MissionAuthorityWithdrawn>>;
}
pub struct WithdrawalOutcome {
    pub local: StopDisposition,
    pub durable: Res<Option<MissionAuthorityWithdrawn>>,
}
pub fn withdraw(
    host: &mut dyn LocalStop,
    writer: &mut dyn RestrictedWithdrawal,
    request: &WithdrawalRequest,
    operation: OperationId,
    recover: bool,
) -> WithdrawalOutcome {
    let local = if recover {
        StopDisposition::default()
    } else {
        host.stop(StopTarget::Campaign {
            engagement_id: request.engagement_id.0,
            campaign_id: request.campaign_id.0,
        })
        .unwrap_or_default()
    };
    // Construction/connection of the writer is deferred by the composition port.
    WithdrawalOutcome {
        local,
        durable: writer.withdraw(request, operation, recover),
    }
}

/// The fixed authenticated host fallback is independent from SQL and Broker state.
pub struct HostStop;
impl LocalStop for HostStop {
    fn stop(&mut self, target: StopTarget) -> Res<StopDisposition> {
        #[cfg(target_os = "linux")]
        if let Ok(Some(disposition)) = super::session::signal(target) {
            return Ok(disposition);
        }
        super::socket::stop(target)
    }
}
