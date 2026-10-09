use crate::mission::{CampaignId, EngagementId};
use crate::Res;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Prepared,
    Released,
}

pub trait SessionFence {
    fn prepare_m1_session(&mut self, engagement: EngagementId, campaign: CampaignId) -> Res<u64>;
    fn recover_m1_session(&mut self, engagement: EngagementId, campaign: CampaignId) -> Res<u64>;
    fn release_m1_session(
        &mut self,
        engagement: EngagementId,
        campaign: CampaignId,
        generation: u64,
    ) -> Res<()>;
}
