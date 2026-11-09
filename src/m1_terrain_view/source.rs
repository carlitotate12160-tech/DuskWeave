//! Source eligibility follows the admitted inference mode.
use super::*;

impl TerrainPublicView {
    pub(super) fn validate_source_mode(&self, mission: &MissionRegistered) -> Res<()> {
        let mode = mission
            .fields
            .m1_inference_permission()
            .and_then(|p| p.autonomous_binding())
            .map(|(_, m)| m);
        let source = match mode {
            Some("controlled_fixture") => TerrainSourceKind::FixtureDnsAcquisition,
            Some("live") => TerrainSourceKind::IssuedDnsAcquisition,
            None => TerrainSourceKind::OperatorReviewedDns,
            _ => return Err(Fail::Input("terrain_source_mode_mismatch")),
        };
        if self.claims.iter().any(|c| c.source_kind != source) {
            return Err(Fail::Input("terrain_source_mode_mismatch"));
        }
        Ok(())
    }
}
