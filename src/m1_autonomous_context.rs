//! Cognition's bounded run feedback projection. Inputs are committed owner views.
use crate::mission::{CampaignId, EngagementId};
use crate::{Fail, Res};
use serde_json::{Value, json};
use uuid::Uuid;

pub struct RunContextBinding {
    pub engagement_id: EngagementId,
    pub campaign_id: CampaignId,
    pub run_profile: String,
    pub execution_mode: String,
    pub inference_profile: Option<String>,
    pub release_sha256: String,
    pub qualification_ref: Uuid,
}
pub struct AutonomousContext {
    pub binding: RunContextBinding,
    pub predecessor: Option<Value>,
}
impl AutonomousContext {
    pub(crate) fn extend(&self, pack: &mut Value) -> Res<()> {
        self.binding.validate()?;
        pack["version"] = json!(if self.binding.inference_profile.is_some() {
            "m1_context_pack_autonomous_v2"
        } else {
            "m1_context_pack_autonomous_v1"
        });
        if let Some(profile) = &self.binding.inference_profile {
            pack["inference_profile"] = json!(profile);
        }
        pack["execution_mode"] = json!(self.binding.execution_mode);
        pack["run_profile"] = json!(self.binding.run_profile);
        pack["pending_unknown"] = json!(["unloaded_owner_views_not_loaded_by_profile"]);
        pack["omissions"]["manifests"] = json!("qualified_profiles_only");
        pack["manifests"] = json!([{"availability":"qualified","execution_mode":self.binding.execution_mode,
            "profile":self.binding.effect_profile(),"qualification_ref":self.binding.qualification_ref,
            "reference":"dns_udp","release_sha256":self.binding.release_sha256,
            "schema":"m1_dns_ipc_v1","transport":"udp"}]);
        let refs = pack["references"]
            .as_array_mut()
            .ok_or(Fail::Store("context_encode"))?;
        for reference in refs {
            if reference["source"] == "caller_declared_premise" {
                reference["source"] = json!("admitted_operator_seed");
            }
        }
        self.attach_feedback(pack)
    }
    fn attach_feedback(&self, pack: &mut Value) -> Res<()> {
        let Some(prior) = &self.predecessor else {
            pack["predecessor"] = json!({"availability":"absent","reason":"first_decision"});
            return Ok(());
        };
        let mut bounded = prior.clone();
        let metadata = feedback_reference(&mut bounded)?;
        bind_selected_seed(&mut bounded, pack)?;
        pack["predecessor"] = bounded;
        pack["references"].as_array_mut().ok_or(Fail::Store("context_encode"))?.push(json!({
            "id":"previous_decision","owner":"campaign_trajectory","source":"cognition",
            "entity":metadata["entity"],"event":metadata["event"],"recorded_at":metadata["recorded_at"],"status":"committed_historical_decision"}));
        set_trajectory_view(pack, prior.get("decision_event").cloned())?;
        fit_optional_summary(pack)?;
        Ok(())
    }
}
fn fit_optional_summary(pack: &mut Value) -> Res<()> {
    if pack["version"] != "m1_context_pack_autonomous_v2"
        || pack["predecessor"].get("hypothesis_summary").is_none()
    {
        return Ok(());
    }
    let raw = serde_json::to_vec(pack).map_err(|_| Fail::Store("context_encode"))?;
    let fits = raw.len() <= 8192
        && [None, Some(&["unexpected_view_request"; 8][..])]
            .into_iter()
            .all(|repair| crate::m1_reasoning::request::bind(&raw, repair).is_ok());
    if !fits {
        pack["predecessor"]
            .as_object_mut()
            .ok_or(Fail::Store("context_feedback_invalid"))?
            .remove("hypothesis_summary");
        pack["predecessor"]["hypothesis_summary_omission"] = json!("byte_bound");
    }
    Ok(())
}
fn set_trajectory_view(pack: &mut Value, event: Option<Value>) -> Res<()> {
    let evaluated = pack["evaluated_at"].clone();
    let views = pack["owner_views"]
        .as_array_mut()
        .ok_or(Fail::Store("context_encode"))?;
    let view = views
        .iter_mut()
        .find(|v| v["owner"] == "campaign_trajectory")
        .ok_or(Fail::Store("context_encode"))?;
    *view = json!({"owner":"campaign_trajectory","availability":"bounded_predecessor","revision":1,
        "frontier":event,"evaluated_at":evaluated});
    Ok(())
}

impl RunContextBinding {
    fn effect_profile(&self) -> &'static str {
        match self.execution_mode.as_str() {
            "controlled_fixture" => "dns-routing-fixture-v1",
            crate::m1_run::LIVE_MODE => crate::m1_run::BOUNDARY_EFFECT_PROFILE,
            _ => "dns-routing-v2",
        }
    }
    fn validate(&self) -> Res<()> {
        let pair = if self.inference_profile.is_some() {
            self.successor_binding()
        } else {
            matches!(
                (self.run_profile.as_str(), self.execution_mode.as_str()),
                (
                    "autonomous-dns-fixture-v1" | "autonomous-dns-provider-fixture-v1",
                    "controlled_fixture"
                ) | ("autonomous-dns-v1", "live")
            )
        };
        let valid = [
            pair,
            !self.engagement_id.0.is_nil(),
            !self.campaign_id.0.is_nil(),
            !self.qualification_ref.is_nil(),
            self.release_sha256.len() == 64,
            self.release_sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        ];
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or(Fail::State("context_basis_mismatch"))
    }

    fn successor_binding(&self) -> bool {
        let Some(name) = self.inference_profile.as_deref() else {
            return false;
        };
        let valid = [
            crate::m1_run::successor_mode_for(&self.run_profile)
                == Some(self.execution_mode.as_str()),
            crate::m1_inference_profile::lookup(name).is_ok(),
        ];
        valid.into_iter().all(|v| v)
    }
}

fn feedback_reference(prior: &mut Value) -> Res<Value> {
    let obj = prior
        .as_object_mut()
        .ok_or(Fail::Store("context_feedback_invalid"))?;
    let entity = obj
        .remove("decision_entity")
        .ok_or(Fail::Store("context_feedback_invalid"))?;
    let event = obj
        .remove("decision_event")
        .ok_or(Fail::Store("context_feedback_invalid"))?;
    let recorded = obj
        .remove("recorded_at")
        .ok_or(Fail::Store("context_feedback_invalid"))?;
    Ok(json!({"entity":entity,"event":event,"recorded_at":recorded}))
}
impl AutonomousContext {
    pub(crate) fn selected_seed(&self) -> Option<crate::mission::AssetRef> {
        self.predecessor
            .as_ref()
            .and_then(|p| p["selected_seed_entity"].as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(crate::mission::AssetRef)
    }
}

// Resolve the historical seed identity through this freshly fitted pack's local labels.
fn bind_selected_seed(prior: &mut Value, pack: &Value) -> Res<()> {
    let seed = prior
        .as_object_mut()
        .ok_or(Fail::Store("context_feedback_invalid"))?
        .remove("selected_seed_entity")
        .ok_or(Fail::Store("context_feedback_invalid"))?;
    let selected = pack["references"]
        .as_array()
        .ok_or(Fail::Store("context_encode"))?
        .iter()
        .find(|r| r["owner"] == "operator" && r["entity"] == seed)
        .and_then(|r| r["id"].as_str())
        .ok_or(Fail::State("context_required_seed_missing"))?;
    prior["selected_seed_reference"] = json!(selected);
    Ok(())
}
