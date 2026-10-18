//! Terrain-aware ProposalBasis wrapper over existing Mission Basis and retained dependencies.
use super::inference::Scope;
use super::postgres::Basis;
use super::proposal::ProposalBasis;
use crate::m1_terrain_view::{TerrainDependency, TerrainPublicView, verify_current_dependencies};
use crate::mission::{AssetRef, MissionRegistered};
use crate::postgres_mission::PgContextReader;
use crate::{Fail, postgres_terrain_view};
use postgres::IsolationLevel;
use uuid::Uuid;

pub struct TerrainBasis {
    pub mission: Basis,
    pub dependencies: Vec<TerrainDependency>,
}

impl ProposalBasis for TerrainBasis {
    fn reference_eligible(&self, r: &str) -> bool {
        self.mission.reference_eligible(r)
    }

    fn manifest_eligible(&self, r: &str) -> bool {
        self.mission.manifest_eligible(r)
    }

    fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
        self.mission.current_authority_and_premises()
    }

    fn before_provider_release(&mut self) -> Result<(), &'static str> {
        self.mission.current_authority_and_premises()?;
        let deps = self.dependencies.clone();
        self.verify_dependencies(&deps)
    }

    fn current_authority_for_references(
        &mut self,
        references: &[String],
    ) -> Result<(), &'static str> {
        self.mission.current_authority_and_premises()?;
        let cited = self.cited_dependencies(references)?;
        self.verify_dependencies(&cited)
    }

    fn safe_explanation(&self, text: &str) -> bool {
        self.mission.safe_explanation(text)
    }
}

impl TerrainBasis {
    fn cited_dependencies(
        &self,
        references: &[String],
    ) -> Result<Vec<TerrainDependency>, &'static str> {
        if references.iter().any(|r| !self.reference_allowed(r)) {
            return Err("reference_ineligible");
        }
        Ok(self
            .dependencies
            .iter()
            .filter(|d| references.iter().any(|r| r == &d.local_id))
            .cloned()
            .collect())
    }

    fn reference_allowed(&self, r: &str) -> bool {
        self.mission.reference_eligible(r)
            && (!r.starts_with("terrain_") || self.dependencies.iter().any(|d| d.local_id == r))
    }

    fn verify_dependencies(&mut self, deps: &[TerrainDependency]) -> Result<(), &'static str> {
        if deps.is_empty() {
            return Ok(());
        }
        if deps.iter().any(|d| !self.dependency_in_scope(d)) {
            return Err("reference_ineligible");
        }
        let (assets, claim_ids) = collect_dependency_targets(deps);
        let (view, now) = self.read_current_terrain_view(&assets, claim_ids)?;
        verify_current_dependencies(deps, &view, now)
    }

    fn dependency_in_scope(&self, d: &TerrainDependency) -> bool {
        self.mission.reference_eligible(&d.local_id)
    }

    fn read_current_terrain_view(
        &mut self,
        assets: &[AssetRef],
        claim_ids: Vec<Uuid>,
    ) -> Result<(TerrainPublicView, i64), &'static str> {
        let mut tx = self
            .mission
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .map_err(|_| "current_authority_unavailable")?;
        let (mission, now) = PgContextReader::read_current_publication(
            &mut tx,
            self.mission.scope.engagement,
            self.mission.scope.campaign,
            self.mission.scope.decision,
        )
        .map_err(|_| "current_authority_unavailable")?;
        verify_mission_disclosure(&mission, &self.mission.scope, assets)?;
        let view = postgres_terrain_view::read_in_tx(&mut tx, &mission, assets, Some(claim_ids))
            .map_err(map_terrain_read_err)?;
        tx.commit().map_err(|_| "current_authority_unavailable")?;
        Ok((view, now))
    }
}

fn collect_dependency_targets(deps: &[TerrainDependency]) -> (Vec<AssetRef>, Vec<Uuid>) {
    let mut assets = Vec::new();
    let mut claim_ids = Vec::new();
    for dep in deps {
        if !assets.contains(&dep.asset_ref) {
            assets.push(dep.asset_ref);
        }
        if !claim_ids.contains(&dep.claim_id) {
            claim_ids.push(dep.claim_id);
        }
    }
    (assets, claim_ids)
}

fn verify_mission_disclosure(
    mission: &MissionRegistered,
    scope: &Scope,
    assets: &[AssetRef],
) -> Result<(), &'static str> {
    if !mission_matches_scope(mission, scope) {
        return Err("current_authority_unavailable");
    }
    if assets.iter().any(|a| {
        !mission.fields.included_assets.contains(a) || mission.fields.excluded_assets.contains(a)
    }) {
        return Err("reference_ineligible");
    }
    Ok(())
}

fn mission_matches_scope(mission: &MissionRegistered, scope: &Scope) -> bool {
    let Some(perm) = mission.fields.m1_inference_permission() else {
        return false;
    };
    let d = perm.context_disclosure();
    mission.owner_revision == scope.mission_revision
        && d.permission_id == scope.permission
        && d.owner_qualified_summaries
        && d.operator_premises
}

fn map_terrain_read_err(err: Fail) -> &'static str {
    match err {
        Fail::Input(_) => "reference_ineligible",
        _ => "current_authority_unavailable",
    }
}
