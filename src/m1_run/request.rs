//! Run request identity and closed fixture/LAB admission.
use super::{FIXTURE_MODE, FIXTURE_PROFILE, PROVIDER_FIXTURE_PROFILE, controlled_fixture_profile};
use crate::m1_context_input::ContextRequest;
use crate::mission::AssetRef;
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunSeed {
    pub asset_ref: AssetRef,
    pub name: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunRequest {
    pub version: String,
    pub run_id: Uuid,
    pub run_profile: String,
    pub execution_mode: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_profile"
    )]
    pub inference_profile: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_live_binding"
    )]
    pub live_binding: Option<crate::m1_inference_permission::LiveBinding>,
    pub deadline_at_ms: i64,
    pub context: ContextRequest,
    pub seed_names: Vec<RunSeed>,
}
impl RunRequest {
    pub fn parse(raw: &[u8]) -> Res<Self> {
        if raw.len() > 16384 {
            return Err(Fail::Input("size_limit"));
        }
        let q: Self =
            serde_json::from_slice(raw).map_err(|_| Fail::Input("invalid_run_request"))?;
        q.validate()?;
        Ok(q)
    }

    pub fn validate(&self) -> Res<()> {
        self.context.validate_ingress()?;
        if self.https_loop() {
            return if self.live() {
                self.validate_live()
            } else {
                self.validate_controlled()
            };
        }
        if self.live() {
            return self.validate_live();
        }
        self.validate_controlled()
    }

    fn validate_controlled(&self) -> Res<()> {
        let valid = [
            self.valid_version_profile(),
            self.live_binding.is_none(),
            !self.run_id.is_nil(),
            controlled_fixture_profile(&self.run_profile),
            self.execution_mode == FIXTURE_MODE,
            self.deadline_at_ms > 0,
            (1..=8).contains(&self.seed_names.len()),
            self.seed_names.len() == self.context.candidates.len(),
        ];
        if !valid.into_iter().all(|v| v) {
            return Err(Fail::Input("invalid_run_request"));
        }
        self.validate_seeds()
    }

    pub(super) fn validate_seeds(&self) -> Res<()> {
        for (seed, candidate) in self.seed_names.iter().zip(&self.context.candidates) {
            crate::m1_terrain_input::validate_dns_hostname(&seed.name)?;
            if seed.asset_ref != candidate.asset_ref {
                return Err(Fail::Input("invalid_run_seeds"));
            }
        }
        Ok(())
    }

    pub fn successor(&self) -> bool {
        matches!(
            self.version.as_str(),
            "m1_autonomous_run_request_v2"
                | "m1_autonomous_run_request_v3"
                | "m1_autonomous_run_request_v4"
                | crate::m1_https::VERSION_HTTPS_LOOP
        )
    }

    pub fn https_loop(&self) -> bool {
        self.version == crate::m1_https::VERSION_HTTPS_LOOP
    }

    pub fn supervised_inference(&self) -> bool {
        self.successor() || self.run_profile == super::PROVIDER_FIXTURE_PROFILE
    }

    pub fn timing_profile(&self) -> Res<Option<&'static crate::m1_inference_profile::Timing>> {
        if !self.successor() {
            return Ok(None);
        }
        let name = self
            .inference_profile
            .as_deref()
            .ok_or(Fail::Input("invalid_run_request"))?;
        Ok(Some(crate::m1_inference_profile::lookup(name)?.timing))
    }

    fn valid_version_profile(&self) -> bool {
        match (self.version.as_str(), self.inference_profile.as_deref()) {
            ("m1_autonomous_run_request_v1", None) => {
                matches!(
                    self.run_profile.as_str(),
                    FIXTURE_PROFILE | PROVIDER_FIXTURE_PROFILE
                )
            }
            ("m1_autonomous_run_request_v2", Some(n)) => {
                self.backend_match(n, crate::m1_inference_profile::Backend::Legacy)
            }
            ("m1_autonomous_run_request_v3", Some(n)) => {
                self.backend_match(n, crate::m1_inference_profile::Backend::Genai)
            }
            (crate::m1_https::VERSION_HTTPS_LOOP, Some(n)) => {
                self.run_profile == crate::m1_https::VERSION_HTTPS_LOOP
                    && n == crate::m1_inference_profile::FIXTURE_PRO_V2_PROFILE
            }
            _ => false,
        }
    }

    fn backend_match(&self, name: &str, expected: crate::m1_inference_profile::Backend) -> bool {
        self.run_profile == crate::m1_inference_profile::RUN_PROFILE
            && crate::m1_inference_profile::lookup(name).is_ok_and(|p| p.backend == expected)
    }
}

fn present_profile<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
}
fn present_live_binding<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<crate::m1_inference_permission::LiveBinding>, D::Error> {
    crate::m1_inference_permission::LiveBinding::deserialize(d).map(Some)
}
