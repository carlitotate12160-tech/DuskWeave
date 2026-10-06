#![allow(dead_code)]

use crate::db_support;
use duskweave::input::parse_register;
use duskweave::mission::{CampaignId, EngagementId, RegistrationInput};
use serde_json::{Value, json};
use uuid::Uuid;

pub fn permission() -> Value {
    json!({
        "policy_version": 1, "ct_base_domain": "example.com",
        "provider_disclosure": "crt_sh", "vantage_ref": Uuid::from_u128(0x31),
        "resolver_ipv4": "192.0.2.53",
        "discovery_rules": [{"label_suffix": "example.com"}],
        "contact_rules": [{"asset_ref": Uuid::from_u128(0x21),
            "rule": {"exact": "api.example.net"}, "priority": 1}],
        "excluded_names": [{"exact": "excluded.example.com"}], "approved_path": "/",
        "starts_at": 1700000001, "ends_at": 1700086399,
        "campaign_limits": {"episodes": 2, "provider_calls": 3, "dns_questions": 10,
            "dns_followups": 2, "tcp_connections": 5, "head_requests": 2},
        "concurrency": 1
    })
}

pub fn registration(e: EngagementId, c: CampaignId) -> Value {
    let mut value = db_support::reg_json(e, c);
    value["m1_permission"] = permission();
    value
}

pub fn input(e: EngagementId, c: CampaignId) -> RegistrationInput {
    parse_register(&serde_json::to_vec(&registration(e, c)).unwrap()).unwrap()
}

pub struct InspectPorts {
    pub event: Option<duskweave::mission::MissionRegistered>,
    pub view: duskweave::registration::MissionView,
}

impl duskweave::registration::MissionStore for InspectPorts {
    fn commit_registration(
        &mut self,
        _: &duskweave::mission::Mission,
        _: &duskweave::mission::MissionRegistered,
    ) -> duskweave::Res<duskweave::registration::CommitEffect> {
        Err(duskweave::Fail::State("unexpected_write"))
    }
    fn mission_view(
        &mut self,
        _: EngagementId,
        _: CampaignId,
    ) -> duskweave::Res<Option<duskweave::registration::MissionView>> {
        Ok(Some(self.view.clone()))
    }
    fn outbox_event(
        &mut self,
        _: EngagementId,
        _: CampaignId,
        _: duskweave::mission::OperationId,
    ) -> duskweave::Res<Option<duskweave::mission::MissionRegistered>> {
        Ok(self.event.clone())
    }
}

pub struct InspectHistory;

impl duskweave::registration::TrajectoryPort for InspectHistory {
    fn deliver(
        &mut self,
        _: &duskweave::mission::MissionRegistered,
    ) -> duskweave::Res<duskweave::trajectory::Delivered> {
        Err(duskweave::Fail::State("unexpected_delivery"))
    }
    fn status(
        &mut self,
        _: EngagementId,
        _: CampaignId,
        _: duskweave::mission::EventId,
    ) -> duskweave::Res<duskweave::trajectory::HistoryStatus> {
        Ok(duskweave::trajectory::HistoryStatus::Completed)
    }
}
