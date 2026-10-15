//! Closed preparation ingress: opaque scoped assets and typed priorities only.
use crate::m1_inference_budget::ReservationRequest;
use crate::mission::{AssetRef, OperationId};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    High,
    Medium,
    Low,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub asset_ref: AssetRef,
    pub priority: Priority,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextRequest {
    pub version: String,
    pub mission: ReservationRequest,
    #[serde(deserialize_with = "bounded_candidates")]
    pub candidates: Vec<Candidate>,
}

fn bounded_candidates<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Candidate>, D::Error> {
    struct Candidates;
    impl<'de> serde::de::Visitor<'de> for Candidates {
        type Value = Vec<Candidate>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("at most 64 typed candidates")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut a: A,
        ) -> Result<Self::Value, A::Error> {
            let mut out = Vec::new();
            while let Some(item) = a.next_element()? {
                if out.len() == 64 {
                    return Err(serde::de::Error::custom("candidate_limit"));
                }
                out.push(item);
            }
            Ok(out)
        }
    }
    d.deserialize_seq(Candidates)
}

impl ContextRequest {
    pub fn validate_ingress(&self) -> Res<()> {
        self.mission.validate_basis()?;
        if self.version != "m1_context_prepare_request_v1" {
            return Err(Fail::Input("invalid_context_request"));
        }
        for (i, candidate) in self.candidates.iter().enumerate() {
            if candidate.asset_ref.0.is_nil()
                || self.candidates[..i]
                    .iter()
                    .any(|c| c.asset_ref == candidate.asset_ref)
            {
                return Err(Fail::Input("invalid_context_candidates"));
            }
        }
        Ok(())
    }
    pub fn validate(&self, op: OperationId) -> Res<()> {
        self.mission.validate(op)?;
        self.validate_ingress()
    }
}
