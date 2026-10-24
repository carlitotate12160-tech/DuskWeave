//! Restricted application bookkeeping adapter; operational owners retain their own writes.
use crate::m1_reasoning::proposal::Proposal;
use crate::m1_run::{RunPhase, RunRecord, RunRequest};
use crate::{Fail, Res};
use postgres::Client;
use serde_json::Value;
use uuid::Uuid;

pub struct PgRunStore {
    client: Client,
    pending_diagnostics: Vec<Value>,
}
impl PgRunStore {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            pending_diagnostics: Vec::new(),
        }
    }
    pub fn start(&mut self, q: &RunRequest, release: &str) -> Res<bool> {
        q.validate()?;
        let raw = serde_json::to_value(q).map_err(|_| Fail::Input("invalid_run_request"))?;
        self.client
            .query_one("SELECT execution.run_start($1,$2)", &[&raw, &release])
            .map_err(|_| Fail::State("run_start_refused"))?
            .try_get(0)
            .map_err(|_| Fail::Store("run_decode"))
    }
    pub fn inspect(&mut self, r: Uuid) -> Res<RunRecord> {
        let raw: Option<Value> = self
            .client
            .query_one("SELECT execution.run_record($1)", &[&r])
            .map_err(|_| Fail::Store("run_inspect_unknown"))?
            .try_get(0)
            .map_err(|_| Fail::Store("run_decode"))?;
        serde_json::from_value(raw.ok_or(Fail::State("run_identity_unavailable"))?)
            .map_err(|_| Fail::Store("run_decode"))
    }
    pub fn check(&mut self, r: Uuid) -> Res<()> {
        self.client
            .execute("SELECT execution.run_current($1)", &[&r])
            .map_err(|_| Fail::State("run_stopped"))?;
        Ok(())
    }
    pub fn renew(&mut self, r: Uuid) -> Res<()> {
        self.client
            .execute("SELECT execution.run_renew($1)", &[&r])
            .map_err(|_| Fail::State("run_stopped"))?;
        Ok(())
    }
    pub fn start_phase(&mut self, r: Uuid, p: RunPhase) -> Res<()> {
        self.client
            .execute("SELECT execution.run_phase_start($1,$2)", &[&r, &p.name()])
            .map_err(|_| Fail::State("run_phase_refused"))?;
        Ok(())
    }
    pub fn claim_phase(&mut self, r: Uuid, p: RunPhase, release: &str) -> Res<RunRecord> {
        self.client
            .execute(
                "SELECT execution.run_phase_claim($1,$2,$3)",
                &[&r, &p.name(), &release],
            )
            .map_err(|_| Fail::State("run_child_refused"))?;
        self.inspect(r)
    }
    pub fn complete_phase(&mut self, r: Uuid, p: RunPhase) -> Res<()> {
        self.client
            .execute(
                "SELECT execution.run_phase_complete($1,$2)",
                &[&r, &p.name()],
            )
            .map_err(|_| Fail::State("run_publication_pending"))?;
        Ok(())
    }
    pub fn unknown(&mut self, r: Uuid, p: RunPhase) -> Res<()> {
        self.client
            .execute("SELECT execution.run_unknown($1,$2)", &[&r, &p.name()])
            .map_err(|_| Fail::Store("run_terminal_unknown"))?;
        Ok(())
    }
    pub fn stop(&mut self, r: Uuid) -> Res<()> {
        self.client
            .execute("SELECT execution.run_stop($1)", &[&r])
            .map_err(|_| Fail::Store("run_stop_unknown"))?;
        Ok(())
    }
    pub fn summary(&mut self, r: Uuid) -> Res<Value> {
        let pending: Vec<_> = self
            .pending_diagnostics
            .iter()
            .filter(|v| v["run_id"] == r.to_string())
            .cloned()
            .collect();
        let result = self.durable_summary(r);
        let mut summary = match result {
            Ok(summary) => summary,
            Err(_) if !pending.is_empty() => serde_json::json!({
                "kind":"m1_autonomous_run_receipt_v1", "run_id":r,
                "status":"unavailable", "diagnostics":[],
            }),
            Err(error) => return Err(error),
        };
        if !pending.is_empty() {
            summary["pending_diagnostics"] = pending.into();
            summary["diagnostic_publication"] = "pending_or_unknown".into();
        }
        Ok(summary)
    }
    fn durable_summary(&mut self, r: Uuid) -> Res<Value> {
        self.client
            .query_one("SELECT execution.run_summary($1)", &[&r])
            .map_err(|_| Fail::Store("run_inspect_unknown"))?
            .try_get(0)
            .map_err(|_| Fail::Store("run_decode"))
    }
    pub fn publish_diagnostic(&mut self, diagnostic: &crate::m1_run::PhaseDiagnostic) -> Res<()> {
        self.defer_diagnostic(diagnostic)?;
        let raw = serde_json::to_value(diagnostic)
            .map_err(|_| Fail::Store("run_diagnostic_unavailable"))?;
        self.client
            .execute("SELECT execution.run_phase_diagnostic($1)", &[&raw])
            .map_err(|_| Fail::Unresolved("run_diagnostic_pending"))?;
        self.pending_diagnostics.retain(|v| v != &raw);
        Ok(())
    }
    pub fn defer_diagnostic(&mut self, diagnostic: &crate::m1_run::PhaseDiagnostic) -> Res<()> {
        diagnostic.validate()?;
        let raw = serde_json::to_value(diagnostic)
            .map_err(|_| Fail::Store("run_diagnostic_unavailable"))?;
        if let Some(old) = self
            .pending_diagnostics
            .iter()
            .find(|v| v["run_id"] == raw["run_id"] && v["phase"] == raw["phase"])
        {
            return (old == &raw)
                .then_some(())
                .ok_or(Fail::State("run_diagnostic_conflict"));
        }
        self.pending_diagnostics.push(raw);
        Ok(())
    }
    pub fn first_proposal(&mut self, record: &RunRecord) -> Res<Proposal> {
        if self.first_disposition(record)? != "validated_proposal" {
            return Err(Fail::State("run_proposal_ineligible"));
        }
        let q = &record.request.context.mission;
        let raw:Value=self.client.query_one("SELECT payload->'proposal' FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2 AND d=$3",
            &[&q.engagement_id.0,&q.campaign_id.0,&record.decision_one])
            .map_err(|_|Fail::Store("run_decision_unavailable"))?.try_get(0).map_err(|_|Fail::Store("run_decode"))?;
        serde_json::from_value(raw).map_err(|_| Fail::State("run_proposal_unavailable"))
    }
    pub fn first_step(&mut self, record: &RunRecord) -> Res<crate::m1_run::NextStep> {
        if self.first_disposition(record)? == "recorded_stale_premises" {
            return Ok(crate::m1_run::NextStep::Terminal("deferred"));
        }
        let proposal = self.first_proposal(record)?;
        crate::m1_run::next_step(&proposal, record.request.seed_names.len())
    }
    pub fn first_disposition(&mut self, record: &RunRecord) -> Res<String> {
        let q = &record.request.context.mission;
        self.client.query_one("SELECT payload->>'disposition' FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2 AND d=$3",
            &[&q.engagement_id.0,&q.campaign_id.0,&record.decision_one])
            .map_err(|_|Fail::Store("run_decision_unavailable"))?.try_get(0).map_err(|_|Fail::Store("run_decode"))
    }
    pub fn terminal(&mut self, r: Uuid, reason: &str) -> Res<()> {
        self.client
            .execute("SELECT execution.run_terminal($1,$2)", &[&r, &reason])
            .map_err(|_| Fail::Store("run_terminal_unknown"))?;
        Ok(())
    }
    pub fn predecessor(&mut self, r: Uuid) -> Res<Value> {
        self.client
            .query_one("SELECT execution.run_predecessor($1)", &[&r])
            .map_err(|_| Fail::Store("run_feedback_unavailable"))?
            .try_get(0)
            .map_err(|_| Fail::Store("run_decode"))
    }
    pub fn reserve_effect(
        &mut self,
        r: Uuid,
        index: usize,
    ) -> Res<crate::m1_terrain_input::TerrainDnsRequest> {
        let index = i32::try_from(index).map_err(|_| Fail::Input("invalid_run_seeds"))?;
        let raw: Value = self
            .client
            .query_one("SELECT execution.reserve_fixture_dns($1,$2)", &[&r, &index])
            .map_err(|_| Fail::State("fixture_release_refused"))?
            .try_get(0)
            .map_err(|_| Fail::Store("run_decode"))?;
        serde_json::from_value(raw).map_err(|_| Fail::Store("run_decode"))
    }
    pub fn commit_result(
        &mut self,
        r: Uuid,
        request: &crate::m1_terrain_input::TerrainDnsRequest,
    ) -> Res<()> {
        let raw = serde_json::to_value(request).map_err(|_| Fail::Store("run_decode"))?;
        let mut tx = self
            .client
            .transaction()
            .map_err(|_| Fail::Unresolved("fixture_result_unknown"))?;
        tx.execute("SELECT execution.commit_fixture_dns($1,$2)", &[&r, &raw])
            .map_err(|_| Fail::Unresolved("fixture_result_unknown"))?;
        tx.commit()
            .map_err(|_| Fail::Unresolved("fixture_result_unknown"))
    }
}
