//! Typed DNS/HEAD effect reservation, commit, refresh, and outcome ports for `m1_https_loop_v1`.
use super::PgRunStore;
use super::https_loop::RefreshDecision;
use crate::m1_https::{AdmittedHttpsHeadRequest, HttpsHeadResult};
use crate::m1_terrain_input::TerrainDnsRequest;
use crate::{Fail, Res};
use serde_json::Value;
use uuid::Uuid;

impl PgRunStore {
    pub fn reserve_https_dns(
        &mut self,
        run_id: Uuid,
        step_index: usize,
        attempt_kind: &str,
        seed_index: usize,
    ) -> Res<TerrainDnsRequest> {
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        let seed = i32::try_from(seed_index).map_err(|_| Fail::Input("invalid_run_seeds"))?;
        let raw: Value = self
            .client
            .query_one(
                "SELECT execution.reserve_https_loop_dns($1,$2,$3,$4)",
                &[&run_id, &idx, &attempt_kind, &seed],
            )
            .map_err(classify_reserve_error)?
            .try_get(0)
            .map_err(|_| Fail::Store("run_decode"))?;
        serde_json::from_value(raw).map_err(|_| Fail::Store("run_decode"))
    }

    pub fn commit_https_dns(
        &mut self,
        run_id: Uuid,
        step_index: usize,
        attempt_kind: &str,
        req: &TerrainDnsRequest,
    ) -> Res<()> {
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        let raw = serde_json::to_value(req).map_err(|_| Fail::Store("run_decode"))?;
        let mut tx = self
            .client
            .transaction()
            .map_err(|_| Fail::Unresolved("fixture_result_unknown"))?;
        tx.execute(
            "SELECT execution.commit_https_loop_dns($1,$2,$3,$4)",
            &[&run_id, &idx, &attempt_kind, &raw],
        )
        .map_err(|_| Fail::Unresolved("fixture_result_unknown"))?;
        tx.commit()
            .map_err(|_| Fail::Unresolved("fixture_result_unknown"))
    }

    pub fn reserve_https_head(
        &mut self,
        run_id: Uuid,
        step_index: usize,
    ) -> Res<AdmittedHttpsHeadRequest> {
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        let raw: Value = self
            .client
            .query_one(
                "SELECT execution.reserve_https_loop_head($1,$2)",
                &[&run_id, &idx],
            )
            .map_err(classify_reserve_error)?
            .try_get(0)
            .map_err(|_| Fail::Store("run_decode"))?;
        serde_json::from_value(raw).map_err(|_| Fail::Store("run_decode"))
    }

    pub fn commit_https_head(
        &mut self,
        run_id: Uuid,
        step_index: usize,
        result: &HttpsHeadResult,
    ) -> Res<()> {
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        let raw = serde_json::to_value(result).map_err(|_| Fail::Store("run_decode"))?;
        let mut tx = self
            .client
            .transaction()
            .map_err(|_| Fail::Unresolved("fixture_result_unknown"))?;
        tx.execute(
            "SELECT execution.commit_https_loop_head($1,$2,$3)",
            &[&run_id, &idx, &raw],
        )
        .map_err(|_| Fail::Unresolved("fixture_result_unknown"))?;
        tx.commit()
            .map_err(|_| Fail::Unresolved("fixture_result_unknown"))
    }

    pub fn block_https_head(&mut self, run_id: Uuid, step_index: usize, reason: &str) -> Res<()> {
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        self.client
            .execute(
                "SELECT execution.block_https_loop_head($1,$2,$3)",
                &[&run_id, &idx, &reason],
            )
            .map_err(|_| Fail::Store("run_terminal_unknown"))?;
        Ok(())
    }

    pub fn mark_https_effect_unknown(
        &mut self,
        run_id: Uuid,
        step_index: usize,
        effect_kind: &str,
        reason: &str,
    ) -> Res<()> {
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        self.client
            .execute(
                "SELECT execution.mark_https_loop_effect_unknown($1,$2,$3,$4)",
                &[&run_id, &idx, &effect_kind, &reason],
            )
            .map_err(|_| Fail::Store("run_terminal_unknown"))?;
        Ok(())
    }

    pub fn close_https_effect_session(&mut self, run_id: Uuid, disposition: &str) -> Res<()> {
        self.client
            .execute(
                "SELECT execution.close_https_loop_effect_session($1,$2)",
                &[&run_id, &disposition],
            )
            .map_err(|_| Fail::Store("run_terminal_unknown"))?;
        Ok(())
    }

    pub fn https_refresh_decision(
        &mut self,
        run_id: Uuid,
        step_index: usize,
    ) -> Res<RefreshDecision> {
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        let raw: Value = self
            .client
            .query_one(
                "SELECT execution.https_loop_refresh_check($1,$2)",
                &[&run_id, &idx],
            )
            .map_err(classify_reserve_error)?
            .try_get(0)
            .map_err(|_| Fail::Store("run_decode"))?;
        decode_refresh_decision(&raw)
    }

    pub fn read_committed_https_result(
        &mut self,
        run_id: Uuid,
        step_index: usize,
    ) -> Res<Option<HttpsHeadResult>> {
        let idx = i32::try_from(step_index).map_err(|_| Fail::Input("https_loop_step_invalid"))?;
        let Some(raw) = self.read_raw_https_result(run_id, idx)? else {
            return Ok(None);
        };
        let result: HttpsHeadResult =
            serde_json::from_value(raw).map_err(|_| Fail::Store("run_decode"))?;
        result.validate().map_err(|_| Fail::Store("run_decode"))?;
        Ok(Some(result))
    }

    fn read_raw_https_result(&mut self, run_id: Uuid, idx: i32) -> Res<Option<Value>> {
        let has_reader: bool = self
            .client
            .query_one(
                "SELECT pg_catalog.to_regprocedure('execution.read_https_loop_head_result(uuid,integer)') IS NOT NULL",
                &[],
            )
            .map_err(|_| Fail::Store("run_inspect_unknown"))?
            .get(0);
        if has_reader {
            return self
                .client
                .query_one(
                    "SELECT execution.read_https_loop_head_result($1,$2)",
                    &[&run_id, &idx],
                )
                .map_err(|_| Fail::Store("run_inspect_unknown"))?
                .try_get(0)
                .map_err(|_| Fail::Store("run_decode"));
        }
        self.client
            .query_opt(
                "SELECT r.result_payload FROM execution.fixture_https_results r \
                 WHERE r.run_id=$1 AND r.step_index=$2",
                &[&run_id, &idx],
            )
            .map_err(|_| Fail::Store("run_inspect_unknown"))?
            .map(|r| r.try_get(0))
            .transpose()
            .map_err(|_| Fail::Store("run_decode"))
    }
}

const KNOWN_RESERVE_REFUSALS: &[(&str, &str)] = &[
    ("fixture_policy_denied", "fixture_policy_denied"),
    ("fixture_budget_exhausted", "fixture_budget_exhausted"),
    ("fixture_time_insufficient", "fixture_time_insufficient"),
    ("fixture_premise_ineligible", "fixture_premise_ineligible"),
    ("fixture_refresh_exhausted", "fixture_refresh_exhausted"),
    ("fixture_head_exhausted", "fixture_head_exhausted"),
    ("fixture_dns_exhausted", "fixture_dns_exhausted"),
    ("fixture_hostname_mismatch", "fixture_hostname_mismatch"),
    ("fixture_release_refused", "fixture_release_refused"),
    ("fixture_scope_refused", "fixture_scope_refused"),
    ("fixture_source_refused", "fixture_source_refused"),
    ("live_policy_denied", "live_policy_denied"),
    ("live_budget_exhausted", "live_budget_exhausted"),
    ("live_time_insufficient", "live_time_insufficient"),
    ("live_premise_ineligible", "live_premise_ineligible"),
    ("live_refresh_exhausted", "live_refresh_exhausted"),
    ("live_head_exhausted", "live_head_exhausted"),
    ("live_dns_exhausted", "live_dns_exhausted"),
    ("live_effect_expired", "live_effect_expired"),
    (
        "live_qualification_unavailable",
        "live_qualification_unavailable",
    ),
    ("live_sender_not_closed", "live_sender_not_closed"),
    ("live_effect_scope_refused", "live_effect_scope_refused"),
    ("live_effect_session_busy", "live_effect_session_busy"),
    ("live_effect_refused", "live_effect_refused"),
    ("https_loop_step_not_found", "https_loop_step_not_found"),
];

fn classify_reserve_error(err: postgres::Error) -> Fail {
    let Some(db) = err.as_db_error().filter(|d| d.code().code() == "P0001") else {
        return Fail::Store("fixture_reserve_unknown");
    };
    KNOWN_RESERVE_REFUSALS
        .iter()
        .find_map(|(k, v)| (*k == db.message()).then_some(Fail::State(v)))
        .unwrap_or(Fail::Store("fixture_reserve_unknown"))
}

fn decode_refresh_decision(raw: &Value) -> Res<RefreshDecision> {
    match raw["decision"].as_str() {
        Some("need_refresh") => {
            let seed = raw["seed_index"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or(Fail::Store("run_decode"))?;
            Ok(RefreshDecision::NeedRefresh { seed_index: seed })
        }
        Some("not_needed") => Ok(RefreshDecision::NotNeeded),
        Some("blocked") => Ok(RefreshDecision::Blocked(
            raw["reason"]
                .as_str()
                .unwrap_or("refresh_blocked")
                .to_owned(),
        )),
        _ => Err(Fail::Store("run_decode")),
    }
}

pub(super) fn stale_refresh_allowed(next: &crate::m1_run::https_loop::HttpsNextStep) -> bool {
    matches!(
        next,
        crate::m1_run::https_loop::HttpsNextStep::Head {
            dependency_refresh: crate::m1_https::DependencyRefresh::DnsOnceIfStale,
            ..
        }
    )
}

pub(super) fn stale_terminal_action(
    step_index: usize,
    next: &crate::m1_run::https_loop::HttpsNextStep,
) -> super::https_loop::StepAction {
    let reason = if matches!(next, crate::m1_run::https_loop::HttpsNextStep::Head { .. }) {
        "blocked_stale_premises"
    } else {
        "deferred"
    };
    super::https_loop::StepAction::Terminal { step_index, reason }
}

pub(super) fn d4_intent_kind(p: &crate::m1_reasoning::proposal::Proposal) -> &'static str {
    use crate::m1_reasoning::proposal::Category;
    match p.next_intent.category {
        Category::Validate => "validate_refused_at_d4",
        Category::ObserveMore => "observe_more_refused_at_d4",
        Category::Retain => "retained",
        Category::Defer => "deferred",
        Category::Stop => "stopped",
        Category::SynthesizeCandidateImplementation => "capability_gap",
    }
}
