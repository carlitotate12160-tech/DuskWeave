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
    pub(crate) fn extend(
        &self,
        pack: &mut Value,
        context_cap: usize,
        configuration_cap: usize,
    ) -> Res<()> {
        self.binding.validate()?;
        pack["version"] = json!(self.binding.pack_version());
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
        let caps = if pack["version"] == "m1_context_pack_autonomous_v3" {
            (context_cap, configuration_cap)
        } else {
            (8192, 16384)
        };
        self.attach_feedback(pack, caps)
    }
    fn attach_feedback(&self, pack: &mut Value, caps: (usize, usize)) -> Res<()> {
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
        fit_optional_summary(pack, caps)?;
        Ok(())
    }
}
fn fit_optional_summary(pack: &mut Value, caps: (usize, usize)) -> Res<()> {
    if !matches!(
        pack["version"].as_str(),
        Some("m1_context_pack_autonomous_v2" | "m1_context_pack_autonomous_v3")
    ) || pack["predecessor"].get("hypothesis_summary").is_none()
    {
        return Ok(());
    }
    let raw = serde_json::to_vec(pack).map_err(|_| Fail::Store("context_encode"))?;
    let fits = crate::m1_reasoning::request::capacity_fits(&raw, caps.0, caps.1, 16384);
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
    fn pack_version(&self) -> &'static str {
        use crate::m1_inference_profile::{Backend, lookup};
        match self
            .inference_profile
            .as_deref()
            .and_then(|name| lookup(name).ok())
            .map(|p| p.backend)
        {
            Some(Backend::Genai) => "m1_context_pack_autonomous_v3",
            Some(Backend::Legacy) => "m1_context_pack_autonomous_v2",
            None => "m1_context_pack_autonomous_v1",
        }
    }

    fn effect_profile(&self) -> &'static str {
        if self.execution_mode == "controlled_fixture" {
            "dns-routing-fixture-v1"
        } else {
            "dns-routing-v2"
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
            self.run_profile == crate::m1_inference_profile::RUN_PROFILE,
            self.execution_mode == "controlled_fixture",
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sdk_optional_summary_omission_retains_required_feedback_and_shared_capacity() {
        let id = "00000000-0000-0000-0000-000000000021";
        let required = json!({"availability":"committed","selected_seed_reference":"asset_0",
            "disposition":"validated_proposal","required_owner_feedback":"retained"});
        let mut pack = json!({"version":"m1_context_pack_autonomous_v3","run_profile":"autonomous-dns-fixture-v2",
            "execution_mode":"controlled_fixture","inference_profile":"fixture-genai-responses-v1",
            "engagement_id":id,"campaign_id":id,"decision_id":id,"permission_id":id,"mission_revision":1,
            "references":[{"id":"asset_0"},{"id":"previous_decision"}],"manifests":[{"reference":"dns_udp"}],
            "predecessor":required,"pending_unknown":vec!["x".repeat(900);8]});
        pack["predecessor"]["hypothesis_summary"] = json!("h".repeat(900));
        let raw = serde_json::to_vec(&pack).unwrap();
        assert!(!crate::m1_reasoning::request::capacity_fits(
            &raw, 8192, 16384, 16384
        ));
        fit_optional_summary(&mut pack, (8192, 16384)).unwrap();
        assert!(pack["predecessor"].get("hypothesis_summary").is_none());
        assert_eq!(
            pack["predecessor"]["hypothesis_summary_omission"],
            "byte_bound"
        );
        assert_eq!(
            pack["predecessor"].as_object().unwrap().len(),
            required.as_object().unwrap().len() + 1
        );
        for (key, value) in required.as_object().unwrap() {
            assert_eq!(&pack["predecessor"][key], value);
        }
        let raw = serde_json::to_vec(&pack).unwrap();
        assert!(crate::m1_reasoning::request::capacity_fits(
            &raw, 8192, 16384, 16384
        ));
    }
}
