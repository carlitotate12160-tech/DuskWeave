//! Mission-owned live consent semantics. No transport or host execution dependency.
use super::*;

/// Immutable release/qualification/resolver references; syntax grants no live authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveBinding {
    pub qualification_ref: Uuid,
    pub release_sha256: String,
    pub resolver_ipv4: String,
}
impl<'de> Deserialize<'de> for LiveBinding {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Object;
        impl<'de> serde::de::Visitor<'de> for Object {
            type Value = LiveBinding;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("live binding object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<Self::Value, A::Error> {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Fields {
                    qualification_ref: Uuid,
                    release_sha256: String,
                    resolver_ipv4: String,
                }
                let v = Fields::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(LiveBinding {
                    qualification_ref: v.qualification_ref,
                    release_sha256: v.release_sha256,
                    resolver_ipv4: v.resolver_ipv4,
                })
            }
        }
        d.deserialize_map(Object)
    }
}
impl LiveBinding {
    pub fn validate_for(&self, run_profile: &str) -> Res<()> {
        self.validate()?;
        crate::m1_inference_profile::resolver_binding(run_profile, &self.resolver_ipv4)
            .then_some(())
            .ok_or(Fail::Input("inference_live_binding_refused"))
    }
    pub fn validate(&self) -> Res<()> {
        let resolver = self
            .resolver_ipv4
            .parse::<std::net::Ipv4Addr>()
            .map_err(|_| Fail::Input("inference_live_binding_refused"))?;
        let valid = [
            !self.qualification_ref.is_nil(),
            self.release_sha256.len() == 64,
            self.release_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            resolver.to_string() == self.resolver_ipv4,
        ];
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or(Fail::Input("inference_live_binding_refused"))
    }
}

impl InferencePermission {
    pub(super) fn check_live(&self) -> Res<()> {
        self.live_binding
            .as_ref()
            .ok_or(Fail::Input("inference_live_binding_refused"))?
            .validate_for(self.run_profile.as_deref().unwrap_or_default())?;
        let profile = crate::m1_inference_profile::lookup(&self.profile)?;
        let valid = [
            crate::m1_inference_profile::live_binding(
                self.run_profile.as_deref().unwrap_or_default(),
                self.execution_mode.as_deref().unwrap_or_default(),
                &self.profile,
            ),
            self.recipient == profile.connection.recipient,
            self.provider_terms_approved,
            !self.permission_id.is_nil(),
            !self.provider_terms_ref.is_nil(),
            self.operation_limits.request_bytes == 16384,
            self.field_classes
                .contains(&SafeFieldClass::AdmittedCampaignDecisionFeedback),
            self.field_classes
                .contains(&SafeFieldClass::AdmittedRepairFieldsCodes),
        ];
        if !valid.into_iter().all(|v| v) {
            return Err(Fail::Input("inference_unsupported_profile"));
        }
        self.check_field_classes()?;
        self.validate_native_profile()
    }
    pub fn live_binding(&self) -> Option<&LiveBinding> {
        self.live_binding.as_ref()
    }
}
