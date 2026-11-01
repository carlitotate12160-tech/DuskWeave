//! Closed source/version identities. Issued labels still require owner verification.
use super::TerrainDnsRequest;
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerrainRequestVersion {
    #[serde(rename = "m1_terrain_dns_observation_v1")]
    M1TerrainDnsObservationV1,
    #[serde(rename = "m1_terrain_dns_issued_observation_v1")]
    M1TerrainDnsIssuedObservationV1,
    #[serde(rename = "m1_terrain_dns_issued_observation_v2")]
    M1TerrainDnsIssuedObservationV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerrainSourceKind {
    #[serde(rename = "operator_reviewed_dns")]
    OperatorReviewedDns,
    #[serde(rename = "fixture_dns_acquisition")]
    FixtureDnsAcquisition,
    #[serde(rename = "issued_dns_acquisition")]
    IssuedDnsAcquisition,
}

impl TerrainDnsRequest {
    pub(super) fn validate_source(&self) -> Res<()> {
        use TerrainRequestVersion::*;
        use TerrainSourceKind::*;
        matches!(
            (self.version, self.source_kind),
            (M1TerrainDnsObservationV1, OperatorReviewedDns)
                | (M1TerrainDnsIssuedObservationV1, FixtureDnsAcquisition)
                | (M1TerrainDnsIssuedObservationV2, IssuedDnsAcquisition)
        )
        .then_some(())
        .ok_or(Fail::Input("invalid_source_binding"))
    }
}
