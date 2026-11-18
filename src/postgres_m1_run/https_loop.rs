//! Typed execution SQL ports for the indexed `m1_https_loop_v1` runner.
use super::PgRunStore;
use super::https_loop_effects::{d4_intent_kind, stale_refresh_allowed, stale_terminal_action};
use crate::m1_https::{DecisionEndpointMap, DependencyRefresh};
use crate::m1_reasoning::proposal::Proposal;
use crate::m1_run::https_loop::{HttpsNextStep, classify_next_step};
use crate::m1_run::{RunPhase, RunRecord};
use crate::{Fail, Res};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct HttpsLoopStep {
    pub run_id: Uuid,
    pub step_index: i32,
    pub decision_id: Uuid,
    pub next_decision_id: Option<Uuid>,
    pub decision_phase: String,
    pub acquisition_phase: Option<String>,
    pub status: String,
    pub protection_state: String,
    pub next_intent_kind: Option<String>,
    pub resolved_asset_ref: Option<Uuid>,
    pub resolved_hostname: Option<String>,
    pub resolved_ipv4: Option<String>,
    pub resolved_endpoint_ref: Option<String>,
    pub resolved_path: Option<String>,
    pub parent_claim_id: Option<Uuid>,
    pub parent_claim_revision: Option<i32>,
    pub dependency_refresh: Option<String>,
    pub episode_index: Option<i32>,
    pub dns_attempt_id: Option<Uuid>,
    pub dns_result_id: Option<Uuid>,
    pub refresh_attempt_id: Option<Uuid>,
    pub refresh_result_id: Option<Uuid>,
    pub refresh_outcome: Option<String>,
    pub head_attempt_id: Option<Uuid>,
    pub head_result_id: Option<Uuid>,
    pub head_outcome: Option<String>,
    pub terrain_operation_id: Option<Uuid>,
    pub head_terrain_operation_id: Option<Uuid>,
    pub disposition: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RefreshDecision {
    NeedRefresh { seed_index: usize },
    NotNeeded,
    Blocked(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum StepAction {
    AcquireDns {
        step_index: usize,
        seed_index: usize,
    },
    AcquireHead {
        step_index: usize,
    },
    Terminal {
        step_index: usize,
        reason: &'static str,
    },
    CompletedAtDecisionFour,
}

impl PgRunStore {
    pub fn https_step(&mut self, run_id: Uuid, step_index: usize) -> Res<HttpsLoopStep> {
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        let raw: Value = self
            .client
            .query_one(
                "SELECT execution.https_loop_step_record($1,$2)",
                &[&run_id, &idx],
            )
            .map_err(|_| Fail::Store("run_inspect_unknown"))?
            .try_get(0)
            .map_err(|_| Fail::Store("run_decode"))?;
        serde_json::from_value(raw).map_err(|_| Fail::Store("run_decode"))
    }

    pub fn https_steps(&mut self, run_id: Uuid) -> Res<Vec<HttpsLoopStep>> {
        let raw: Value = self
            .client
            .query_one("SELECT execution.https_loop_steps_for_run($1)", &[&run_id])
            .map_err(|_| Fail::Store("run_inspect_unknown"))?
            .try_get(0)
            .map_err(|_| Fail::Store("run_decode"))?;
        serde_json::from_value(raw).map_err(|_| Fail::Store("run_decode"))
    }

    pub fn https_step_for_phase(&mut self, run_id: Uuid, phase: RunPhase) -> Res<HttpsLoopStep> {
        let steps = self.https_steps(run_id)?;
        let name = phase.name();
        steps
            .into_iter()
            .find(|s| s.decision_phase == name || s.acquisition_phase.as_deref() == Some(name))
            .ok_or(Fail::State("run_phase_refused"))
    }

    pub fn https_predecessor(&mut self, run_id: Uuid, step_index: usize) -> Res<Option<Value>> {
        if step_index <= 1 {
            return Ok(None);
        }
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        let raw: Value = self
            .client
            .query_one(
                "SELECT execution.https_loop_step_predecessor($1,$2)",
                &[&run_id, &idx],
            )
            .map_err(|_| Fail::State("run_feedback_unavailable"))?
            .try_get(0)
            .map_err(|_| Fail::Store("run_decode"))?;
        Ok(if raw.is_null() { None } else { Some(raw) })
    }

    pub fn bind_https_input(
        &mut self,
        e: Uuid,
        c: Uuid,
        d: Uuid,
        run_id: Uuid,
        input_hash: &str,
        map: &DecisionEndpointMap,
    ) -> Res<()> {
        map.validate()?;
        let map_json = serde_json::to_value(map).map_err(|_| Fail::Store("run_decode"))?;
        self.client
            .execute(
                "SELECT execution.bind_https_loop_input($1,$2,$3,$4,$5,$6,$7)",
                &[
                    &e,
                    &c,
                    &d,
                    &run_id,
                    &input_hash,
                    &map.pack_digest,
                    &map_json,
                ],
            )
            .map_err(|_| Fail::State("inference_input_conflict"))?;
        Ok(())
    }

    pub fn read_https_endpoint_map(
        &mut self,
        e: Uuid,
        c: Uuid,
        d: Uuid,
    ) -> Res<Option<DecisionEndpointMap>> {
        let raw: Option<Value> = self
            .client
            .query_one(
                "SELECT execution.read_https_endpoint_map($1,$2,$3)",
                &[&e, &c, &d],
            )
            .map_err(|_| Fail::Store("run_inspect_unknown"))?
            .try_get(0)
            .map_err(|_| Fail::Store("run_decode"))?;
        raw.map(|v| serde_json::from_value(v).map_err(|_| Fail::Store("run_decode")))
            .transpose()
    }

    fn read_step_decision(
        &mut self,
        run: &RunRecord,
        decision_id: Uuid,
    ) -> Res<(String, Proposal)> {
        let q = &run.request.context.mission;
        let row = self
            .client
            .query_one(
                "SELECT payload->>'disposition', payload->'proposal' \
                 FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2 AND d=$3",
                &[&q.engagement_id.0, &q.campaign_id.0, &decision_id],
            )
            .map_err(|_| Fail::Store("run_decision_unavailable"))?;
        let disposition: String = row.try_get(0).map_err(|_| Fail::Store("run_decode"))?;
        let prop_val: Value = row.try_get(1).map_err(|_| Fail::Store("run_decode"))?;
        let proposal: Proposal = serde_json::from_value(prop_val)
            .map_err(|_| Fail::State("run_proposal_unavailable"))?;
        Ok((disposition, proposal))
    }

    pub fn resolve_https_step_action(
        &mut self,
        run: &RunRecord,
        step_index: usize,
    ) -> Res<StepAction> {
        let step = self.https_step(run.request.run_id, step_index)?;
        let (disposition, proposal) = self.read_step_decision(run, step.decision_id)?;
        if disposition != "validated_proposal" && disposition != "recorded_stale_premises" {
            return Err(Fail::State("run_proposal_ineligible"));
        }
        if step_index == 4 {
            return self.record_d4_outcome(run.request.run_id, &proposal);
        }
        self.resolve_active_step(run, &step, step_index, &disposition, &proposal)
    }

    fn resolve_active_step(
        &mut self,
        run: &RunRecord,
        step: &HttpsLoopStep,
        step_index: usize,
        disposition: &str,
        proposal: &Proposal,
    ) -> Res<StepAction> {
        let q = &run.request.context.mission;
        let map =
            self.read_https_endpoint_map(q.engagement_id.0, q.campaign_id.0, step.decision_id)?;
        let next = classify_next_step(proposal, run.request.seed_names.len(), map.as_ref())?;
        if disposition == "recorded_stale_premises" && !stale_refresh_allowed(&next) {
            return Ok(stale_terminal_action(step_index, &next));
        }
        self.persist_step_intent(run, step_index, next, map.as_ref())
    }

    fn record_d4_outcome(&mut self, run_id: Uuid, proposal: &Proposal) -> Res<StepAction> {
        let intent_kind = d4_intent_kind(proposal);
        self.client
            .execute(
                "SELECT execution.record_https_loop_d4_outcome($1,$2)",
                &[&run_id, &intent_kind],
            )
            .map_err(|_| Fail::Store("run_terminal_unknown"))?;
        Ok(StepAction::CompletedAtDecisionFour)
    }

    fn persist_step_intent(
        &mut self,
        run: &RunRecord,
        step_index: usize,
        next: HttpsNextStep,
        map: Option<&DecisionEndpointMap>,
    ) -> Res<StepAction> {
        match next {
            HttpsNextStep::Terminal(reason) => Ok(StepAction::Terminal { step_index, reason }),
            HttpsNextStep::Dns { seed_index } => {
                self.record_dns_intent(run, step_index, seed_index)
            }
            HttpsNextStep::Head {
                asset_ref,
                endpoint_ref,
                dependency_refresh,
            } => self.record_head_intent(
                run,
                step_index,
                asset_ref,
                &endpoint_ref,
                dependency_refresh,
                map,
            ),
        }
    }

    fn record_dns_intent(
        &mut self,
        run: &RunRecord,
        step_index: usize,
        seed_index: usize,
    ) -> Res<StepAction> {
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        let seed = run
            .request
            .seed_names
            .get(seed_index)
            .ok_or(Fail::State("effect_seed_ineligible"))?;
        self.client
            .execute(
                "SELECT execution.record_https_loop_dns_intent($1,$2,$3,$4)",
                &[&run.request.run_id, &idx, &seed.asset_ref.0, &seed.name],
            )
            .map_err(|_| Fail::State("run_phase_refused"))?;
        Ok(StepAction::AcquireDns {
            step_index,
            seed_index,
        })
    }

    fn record_head_intent(
        &mut self,
        run: &RunRecord,
        step_index: usize,
        asset_ref: crate::mission::AssetRef,
        endpoint_ref: &str,
        dependency_refresh: DependencyRefresh,
        map: Option<&DecisionEndpointMap>,
    ) -> Res<StepAction> {
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        let ep = map
            .and_then(|m| m.bindings.iter().find(|b| b.local_id == endpoint_ref))
            .ok_or(Fail::State("endpoint_map_unavailable"))?;
        let rev = i32::try_from(ep.parent_claim_revision)
            .map_err(|_| Fail::Input("invalid_endpoint_binding"))?;
        self.client
            .execute(
                "SELECT execution.record_https_loop_head_intent($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
                &[
                    &run.request.run_id,
                    &idx,
                    &asset_ref.0,
                    &ep.hostname,
                    &ep.ipv4,
                    &ep.local_id,
                    &ep.approved_path,
                    &ep.parent_claim_id,
                    &rev,
                    &dependency_refresh.as_str(),
                ],
            )
            .map_err(|_| Fail::State("run_phase_refused"))?;
        Ok(StepAction::AcquireHead { step_index })
    }

    pub fn protect_next_decision(&mut self, run_id: Uuid, step_index: usize) -> Res<()> {
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        self.client
            .execute(
                "SELECT execution.protect_next_decision_capacity($1,$2)",
                &[&run_id, &idx],
            )
            .map_err(|_| Fail::State("inference_capacity_exhausted"))?;
        Ok(())
    }

    pub fn convert_protected_capacity(&mut self, e: Uuid, c: Uuid, d: Uuid) -> Res<String> {
        self.client
            .query_one(
                "SELECT execution.convert_protected_capacity($1,$2,$3)",
                &[&e, &c, &d],
            )
            .map_err(|_| Fail::State("protected_capacity_unavailable"))?
            .try_get(0)
            .map_err(|_| Fail::Store("run_decode"))
    }

    pub fn release_protected_capacity(
        &mut self,
        run_id: Uuid,
        step_index: usize,
        disposition: &str,
    ) -> Res<()> {
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        self.client
            .execute(
                "SELECT execution.release_protected_capacity($1,$2,$3)",
                &[&run_id, &idx, &disposition],
            )
            .map_err(|_| Fail::Store("run_terminal_unknown"))?;
        Ok(())
    }

    pub fn terminate_https_loop(
        &mut self,
        run_id: Uuid,
        step_index: usize,
        reason: &str,
    ) -> Res<()> {
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        self.client
            .execute(
                "SELECT execution.run_https_loop_terminal($1,$2,$3)",
                &[&run_id, &idx, &reason],
            )
            .map_err(|_| Fail::Store("run_terminal_unknown"))?;
        Ok(())
    }
}
