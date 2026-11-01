use super::{DnsRequest, SCHEMA};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsStatus {
    Completed,
    Limited,
    Timeout,
    Limit,
    ProcessFailure,
    UnsupportedResponse,
    IpcFailure,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DnsAnswer {
    pub ipv4: String,
    pub ttl_seconds: u32,
}

impl<'de> Deserialize<'de> for DnsAnswer {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Object;
        impl<'de> serde::de::Visitor<'de> for Object {
            type Value = DnsAnswer;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("DNS answer object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<Self::Value, A::Error> {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Fields {
                    ipv4: String,
                    ttl_seconds: u32,
                }
                let value = Fields::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(DnsAnswer {
                    ipv4: value.ipv4,
                    ttl_seconds: value.ttl_seconds,
                })
            }
        }
        d.deserialize_map(Object)
    }
}
impl DnsAnswer {
    fn validate(&self) -> Res<()> {
        let ip = self
            .ipv4
            .parse::<Ipv4Addr>()
            .map_err(|_| Fail::Input("dns_result_refused"))?;
        if ip.to_string() != self.ipv4 || self.ttl_seconds > 86400 {
            return Err(Fail::Input("dns_result_refused"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DnsResult {
    pub schema: String,
    pub kind: String,
    pub attempt: Uuid,
    pub session_generation: u64,
    pub sequence: u32,
    pub status: DnsStatus,
    pub observed_at_ms: i64,
    pub rcode: u8,
    pub truncated: bool,
    pub complete: bool,
    pub withheld_aliases: u32,
    #[serde(deserialize_with = "bounded::<_, DnsAnswer, 4>")]
    pub answers: Vec<DnsAnswer>,
    #[serde(deserialize_with = "bounded::<_, String, 4>")]
    pub aliases: Vec<String>,
}

fn bounded<'de, D, T, const N: usize>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Values<T, const N: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const N: usize> serde::de::Visitor<'de> for Values<T, N> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("bounded DNS projection")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = seq.next_element()? {
                if values.len() == N {
                    return Err(serde::de::Error::custom("dns_projection_limit"));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    d.deserialize_seq(Values::<T, N>(std::marker::PhantomData))
}
impl DnsResult {
    pub fn validate(&self, q: &DnsRequest, now_ms: i64) -> Res<()> {
        let valid = [
            self.schema == SCHEMA,
            self.kind == "RESULT",
            self.sequence == 3,
            self.attempt == q.attempt,
            self.session_generation == q.session_generation,
            self.answers.len() <= 4,
            self.aliases.len() <= 4,
            self.withheld_aliases <= 4,
            self.rcode <= 5,
        ];
        if !valid.into_iter().all(|v| v) {
            return Err(Fail::Input("dns_result_refused"));
        }
        self.validate_disposition(q, now_ms)?;
        self.validate_projection(q)
    }
    fn validate_projection(&self, q: &DnsRequest) -> Res<()> {
        self.validate_answers()?;
        self.validate_aliases(q)
    }
    fn validate_answers(&self) -> Res<()> {
        for (index, answer) in self.answers.iter().enumerate() {
            answer.validate()?;
            if self.answers[..index]
                .iter()
                .any(|prior| prior.ipv4 == answer.ipv4)
            {
                return Err(Fail::Input("dns_result_refused"));
            }
        }
        Ok(())
    }
    fn validate_aliases(&self, q: &DnsRequest) -> Res<()> {
        for (index, name) in self.aliases.iter().enumerate() {
            if !q.allowed_names.contains(name)
                || name == &q.name
                || self.aliases[..index].contains(name)
            {
                return Err(Fail::Input("dns_result_refused"));
            }
        }
        Ok(())
    }
    fn validate_disposition(&self, q: &DnsRequest, now_ms: i64) -> Res<()> {
        let received = matches!(self.status, DnsStatus::Completed | DnsStatus::Limited);
        let valid = if received {
            [
                self.observed_at_ms > 0,
                self.observed_at_ms <= now_ms,
                self.observed_at_ms <= q.deadline_at_ms,
                self.complete == (self.status == DnsStatus::Completed),
                !self.truncated || !self.complete,
                self.rcode == 0 || self.answers.is_empty(),
                self.withheld_aliases == 0 || !self.complete,
            ]
        } else {
            [
                self.observed_at_ms == 0,
                !self.complete,
                !self.truncated,
                self.answers.is_empty(),
                self.aliases.is_empty(),
                self.rcode == 0,
                self.withheld_aliases == 0,
            ]
        };
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or(Fail::Input("dns_result_refused"))
    }
}
