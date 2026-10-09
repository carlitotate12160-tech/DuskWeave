use duskweave::m1_session::SessionFence;
use duskweave::mission::{CampaignId, EngagementId};
use duskweave::postgres_m1_session::PgSessionFence;
use duskweave::{Fail, Res};

pub(super) fn cmd_m1_session(args: &[String]) -> Res<()> {
    let mut action = None;
    let mut engagement = None;
    let mut campaign = None;
    let mut generation = None;

    let mut i = 2;
    while i < args.len() {
        let name = args[i].strip_prefix("--").ok_or(Fail::Input("invalid_args"))?;
        let value = args.get(i + 1).filter(|v| !v.starts_with("--")).ok_or(Fail::Input("invalid_args"))?;
        match name {
            "action" => action = Some(value.as_str()),
            "engagement" => engagement = Some(EngagementId::parse(value).ok_or(Fail::Input("invalid_args"))?),
            "campaign" => campaign = Some(CampaignId::parse(value).ok_or(Fail::Input("invalid_args"))?),
            "generation" => generation = Some(value.parse::<u64>().map_err(|_| Fail::Input("invalid_generation"))?),
            _ => return Err(Fail::Input("invalid_args")),
        }
        i += 2;
    }

    let action = action.ok_or(Fail::Input("missing_action"))?;
    let engagement = engagement.ok_or(Fail::Input("invalid_args"))?;
    let campaign = campaign.ok_or(Fail::Input("invalid_args"))?;

    if action == "release" && generation.is_none() {
        return Err(Fail::Input("missing_generation"));
    }

    let client = super::connect()?;
    let mut fence = PgSessionFence::new(client);

    match action {
        "prepare" => {
            let generation = fence.prepare_m1_session(engagement, campaign)?;
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "action": "prepare_m1_session",
                    "result": "prepared",
                    "generation": generation,
                    "campaign_id": campaign.0,
                    "engagement_id": engagement.0,
                }))
                .map_err(|_| Fail::State("receipt_encode"))?
            );
        }
        "recover" => {
            let generation = fence.recover_m1_session(engagement, campaign)?;
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "action": "recover_m1_session",
                    "result": "recovered",
                    "generation": generation,
                    "campaign_id": campaign.0,
                    "engagement_id": engagement.0,
                }))
                .map_err(|_| Fail::State("receipt_encode"))?
            );
        }
        "release" => {
            fence.release_m1_session(engagement, campaign, generation.ok_or(Fail::Input("missing_generation"))?)?;
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "action": "release_m1_session",
                    "result": "released",
                    "campaign_id": campaign.0,
                    "engagement_id": engagement.0,
                }))
                .map_err(|_| Fail::State("receipt_encode"))?
            );
        }
        _ => return Err(Fail::Input("invalid_action")),
    }
    
    Ok(())
}
