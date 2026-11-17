//! The v4 producer's actual wording through the SDK wire builder; loopback only.
use super::*;
use crate::m1_autonomous_context::{AutonomousContext, RunContextBinding};
use crate::mission::{CampaignId, EngagementId};
use uuid::Uuid;

pub(super) fn exercise() {
    let binding = RunContextBinding {
        engagement_id: EngagementId(Uuid::from_u128(11)),
        campaign_id: CampaignId(Uuid::from_u128(12)),
        run_profile: profiles::LIVE_RUN_PROFILE.into(),
        execution_mode: "live".into(),
        inference_profile: Some(profiles::PRO_V2_PROFILE.into()),
        release_sha256: "a".repeat(64),
        qualification_ref: Uuid::from_u128(17),
    };
    let mut pack = json!({"engagement_id":binding.engagement_id,
        "campaign_id":binding.campaign_id,"decision_id":Uuid::from_u128(13),
        "permission_id":Uuid::from_u128(14),"mission_revision":1,
        "references":[{"id":"asset_0","source":"caller_declared_premise"},{"id":"mission"}],
        "candidates":[{"reference":"asset_0","priority":"high","status":"operator_premise"}],
        "omissions":{},"live_binding":{"qualification_ref":binding.qualification_ref,
            "release_sha256":binding.release_sha256,"resolver_ipv4":"1.1.1.1"},
        "qualification_purpose":"LIVE","question":crate::m1_context::QUESTION});
    AutonomousContext {
        binding,
        predecessor: None,
    }
    .extend(&mut pack, 8192, 16384)
    .unwrap();
    assert_eq!(pack["version"], "m1_context_pack_autonomous_v4");
    assert_eq!(
        pack["question"],
        crate::m1_autonomous_context::LIVE_ROUTING_QUESTION
    );
    assert_eq!(pack["references"][0]["source"], "admitted_operator_seed");
    for (call, codes) in [
        (Uuid::from_u128(15), None),
        (
            Uuid::from_u128(16),
            Some(&["unexpected_view_request"; 8][..]),
        ),
    ] {
        // The existing private helper injects a synthetic key and substitutes
        // only the loopback target before polling the SDK future.
        entities::measure_pro_v2_case(profiles::PRO_V2_PROFILE, &pack, call, codes);
    }
}
