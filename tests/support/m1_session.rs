use duskweave::m1_session::{SessionClaim, SessionRequest};
use duskweave::mission::{CampaignId, EngagementId, OperationId, OperatorRef};
use postgres::Client;
use uuid::Uuid;

pub fn broker_client() -> Client {
    crate::registration_db::runtime_client()
}

pub fn operator() -> OperatorRef {
    OperatorRef(Uuid::from_u128(0x11))
}

pub fn valid_request(e: EngagementId, c: CampaignId) -> SessionRequest {
    SessionRequest {
        engagement_id: e,
        campaign_id: c,
        operator_ref: operator(),
        expected_mission_revision: 1,
    }
}

pub fn valid_claim(
    e: EngagementId,
    c: CampaignId,
    op: OperationId,
    reg_op: OperationId,
) -> SessionClaim {
    SessionClaim {
        engagement_id: e,
        campaign_id: c,
        operation_id: op,
        operator_ref: operator(),
        expected_mission_revision: 1,
        registration_operation_id: reg_op,
        contract: crate::registration_db::reg_json(e, c),
    }
}
