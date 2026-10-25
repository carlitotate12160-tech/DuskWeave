//! Terrain-owned versioned issued-result read contract. Labels alone grant no intake authority.
use crate::mission::{CampaignId, EngagementId};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IssuedDnsReference {
    version: String,
    engagement_id: EngagementId,
    campaign_id: CampaignId,
    result_ref: Uuid,
    run_profile: String,
    execution_mode: String,
}
impl IssuedDnsReference {
    pub fn fixture(e: EngagementId, c: CampaignId, result: Uuid) -> Res<Self> {
        Self::controlled_fixture(e, c, result, "autonomous-dns-fixture-v1")
    }
    pub fn controlled_fixture(
        e: EngagementId,
        c: CampaignId,
        result: Uuid,
        run_profile: &str,
    ) -> Res<Self> {
        let q = Self {
            version: "m1_terrain_dns_issued_observation_v1".into(),
            engagement_id: e,
            campaign_id: c,
            result_ref: result,
            run_profile: run_profile.into(),
            execution_mode: "controlled_fixture".into(),
        };
        q.validate()?;
        Ok(q)
    }
    pub fn validate(&self) -> Res<()> {
        let valid = [
            self.version == "m1_terrain_dns_issued_observation_v1",
            matches!(
                self.run_profile.as_str(),
                "autonomous-dns-fixture-v1"
                    | "autonomous-dns-provider-fixture-v1"
                    | "autonomous-dns-fixture-v2"
            ),
            self.execution_mode == "controlled_fixture",
            !self.engagement_id.0.is_nil(),
            !self.campaign_id.0.is_nil(),
            !self.result_ref.is_nil(),
        ];
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or(Fail::Input("invalid_issued_source_reference"))
    }
}
