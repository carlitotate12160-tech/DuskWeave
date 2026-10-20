//! Deterministic Broker-effect adapter for the explicitly bound fixture lane. No network client.
use crate::m1_terrain_input::{
    DnsAnswer, TerrainDnsRequest, TerrainRequestVersion, TerrainSourceKind,
};
use crate::{Fail, Res};

pub fn acquire(mut issued: TerrainDnsRequest) -> Res<TerrainDnsRequest> {
    if (issued.version, issued.source_kind)
        != (
            TerrainRequestVersion::M1TerrainDnsIssuedObservationV1,
            TerrainSourceKind::FixtureDnsAcquisition,
        )
    {
        return Err(Fail::State("fixture_source_refused"));
    }
    // This deterministic snapshot uses its Broker-issued time. A separate worker
    // wall clock must not turn synthetic data into a future-dated observation.
    issued.answers = vec![DnsAnswer {
        ipv4: "192.0.2.80".into(),
        ttl_seconds: 60,
    }];
    issued.validate()?;
    Ok(issued)
}
