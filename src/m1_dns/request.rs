use super::{PROFILE, SCHEMA};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DnsRequest {
    pub schema: String,
    pub kind: String,
    pub attempt: Uuid,
    pub session_generation: u64,
    pub sequence: u32,
    pub engagement_id: Uuid,
    pub campaign_id: Uuid,
    pub decision_id: Uuid,
    pub profile: String,
    pub execution_mode: String,
    pub run_profile: String,
    pub release_sha256: String,
    pub qualification_ref: Uuid,
    pub name: String,
    pub resolver_ipv4: String,
    pub transport: String,
    pub query_id: u16,
    #[serde(deserialize_with = "names")]
    pub allowed_names: Vec<String>,
    pub deadline_at_ms: i64,
    pub max_duration_ms: u32,
}

impl DnsRequest {
    pub fn validate(&self, now_ms: i64) -> Res<()> {
        let valid = [
            self.schema == SCHEMA,
            self.kind == "REQUEST",
            self.sequence == 0,
            self.session_generation > 0,
            self.profile == PROFILE,
            self.execution_mode == "live",
            self.transport == "udp",
            self.deadline_at_ms > now_ms,
            (1..=2000).contains(&self.max_duration_ms),
            hex_digest(&self.release_sha256),
        ];
        if !valid.into_iter().all(|v| v) {
            return Err(Fail::Input("dns_request_refused"));
        }
        if [
            self.attempt,
            self.engagement_id,
            self.campaign_id,
            self.decision_id,
            self.qualification_ref,
        ]
        .into_iter()
        .any(|id| id.is_nil())
        {
            return Err(Fail::Input("dns_request_refused"));
        }
        self.validate_names()?;
        self.validate_resolver()
    }
    fn validate_names(&self) -> Res<()> {
        crate::m1_terrain_input::validate_dns_hostname(&self.name)?;
        if !(1..=8).contains(&self.allowed_names.len()) || !self.allowed_names.contains(&self.name)
        {
            return Err(Fail::Input("dns_request_refused"));
        }
        for (index, name) in self.allowed_names.iter().enumerate() {
            crate::m1_terrain_input::validate_dns_hostname(name)?;
            if self.allowed_names[..index].contains(name) {
                return Err(Fail::Input("dns_request_refused"));
            }
        }
        Ok(())
    }
    fn validate_resolver(&self) -> Res<()> {
        use crate::m1_inference_profile::{LAB_RUN_PROFILE, resolver_binding};
        if !resolver_binding(&self.run_profile, &self.resolver_ipv4) {
            return Err(Fail::Input("dns_request_refused"));
        }
        let allowed = self.run_profile != LAB_RUN_PROFILE
            || self
                .name
                .strip_suffix(".duskweave.test")
                .is_some_and(|s| !s.is_empty());
        allowed
            .then_some(())
            .ok_or(Fail::Input("dns_request_refused"))
    }
}

pub(super) fn hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn names<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    struct Names;
    impl<'de> serde::de::Visitor<'de> for Names {
        type Value = Vec<String>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("bounded DNS names")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = seq.next_element::<String>()? {
                if values.len() == 8 {
                    return Err(serde::de::Error::custom("dns_names_limit"));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    d.deserialize_seq(Names)
}
