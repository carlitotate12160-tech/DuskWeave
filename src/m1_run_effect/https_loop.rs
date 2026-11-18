//! Fixture effect-boundary dispatch and refresh-chain composition for `m1_https_loop_v1`.
use crate::{connect, m1_reasoning_cli, m1_terrain_cli};
use duskweave::m1_https::{
    AdmittedHttpsHeadRequest, HttpsHeadResult, HttpsOutcomeKind, HttpsSourceKind,
    RedirectDisposition, VERSION_HEAD_RESULT,
};
use duskweave::m1_run::{RunPhase, RunRecord};
use duskweave::m1_terrain_input::{DnsAnswer, TerrainDnsRequest};
use duskweave::m1_terrain_issued::IssuedDnsReference;
use duskweave::mission::OperationId;
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::postgres_m1_run::https_loop::{HttpsLoopStep, RefreshDecision};
use duskweave::postgres_m1_terrain::{PgTerrainStore, https::PgHttpsTerrainStore};
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_terrain_history::{record_https_terrain_history, record_terrain_history};
use duskweave::trajectory::HistoryStatus;
use duskweave::{Fail, Res};
use uuid::Uuid;

pub(crate) fn acquire(run: &RunRecord, phase: RunPhase) -> Res<()> {
    let valid =
        run.request.https_loop() && run.request.execution_mode == duskweave::m1_run::FIXTURE_MODE;
    if !valid {
        return Err(Fail::State("fixture_source_refused"));
    }
    let mut broker = PgRunStore::new(m1_reasoning_cli::broker_connection()?);
    let step = broker.https_step_for_phase(run.request.run_id, phase)?;
    let step_idx =
        usize::try_from(step.step_index).map_err(|_| Fail::State("run_phase_refused"))?;
    dispatch_acquire(&mut broker, run, &step, step_idx)
}

fn dispatch_acquire(
    broker: &mut PgRunStore,
    run: &RunRecord,
    step: &HttpsLoopStep,
    step_idx: usize,
) -> Res<()> {
    match step.next_intent_kind.as_deref() {
        Some("dns_udp") => acquire_initial_dns(broker, run, step, step_idx),
        Some("https-head-v1") => acquire_head_chain(broker, run, step, step_idx),
        _ => Err(Fail::State("fixture_effect_not_selected")),
    }
}

fn seed_index_for_step(run: &RunRecord, step: &HttpsLoopStep) -> Res<usize> {
    let asset = step
        .resolved_asset_ref
        .ok_or(Fail::State("fixture_scope_refused"))?;
    run.request
        .seed_names
        .iter()
        .position(|s| s.asset_ref.0 == asset)
        .ok_or(Fail::State("effect_seed_ineligible"))
}

fn acquire_initial_dns(
    broker: &mut PgRunStore,
    run: &RunRecord,
    step: &HttpsLoopStep,
    step_idx: usize,
) -> Res<()> {
    let seed_idx = seed_index_for_step(run, step)?;
    let seed_name = &run.request.seed_names[seed_idx].name;
    let issued =
        reserve_dns_or_mark_unknown(broker, run.request.run_id, step_idx, "initial", seed_idx)?;
    let acquired = duskweave::m1_fixture_dns::acquire_with_answers(
        issued,
        fixture_dns_answers(seed_name, "initial"),
    )?;
    broker.commit_https_dns(run.request.run_id, step_idx, "initial", &acquired)?;
    deliver_initial_dns_and_close(broker, run, step_idx)
}

fn reserve_dns_or_mark_unknown(
    broker: &mut PgRunStore,
    run_id: Uuid,
    step_idx: usize,
    kind: &str,
    seed_idx: usize,
) -> Res<TerrainDnsRequest> {
    match broker.reserve_https_dns(run_id, step_idx, kind, seed_idx) {
        Ok(issued) => Ok(issued),
        Err(Fail::State(code)) => Err(Fail::State(code)),
        Err(other) => {
            let _ = broker.mark_https_effect_unknown(run_id, step_idx, kind, "dns_reserve_unknown");
            Err(other)
        }
    }
}

fn deliver_initial_dns_and_close(
    broker: &mut PgRunStore,
    run: &RunRecord,
    step_idx: usize,
) -> Res<()> {
    let refreshed = broker.https_step(run.request.run_id, step_idx)?;
    let res_id = refreshed
        .dns_result_id
        .ok_or(Fail::Unresolved("run_terrain_history_pending"))?;
    deliver_dns(run, res_id)?;
    broker.close_https_effect_session(run.request.run_id, "completed")
}

fn prepare_head_refresh(broker: &mut PgRunStore, run: &RunRecord, step_idx: usize) -> Res<bool> {
    match broker.https_refresh_decision(run.request.run_id, step_idx) {
        Ok(RefreshDecision::Blocked(reason)) => {
            broker.block_https_head(run.request.run_id, step_idx, &reason)?;
            Ok(false)
        }
        Ok(RefreshDecision::NeedRefresh { seed_index }) => {
            run_refresh(broker, run, step_idx, seed_index)
        }
        Ok(RefreshDecision::NotNeeded) => Ok(true),
        Err(Fail::State(reason)) => {
            broker.block_https_head(run.request.run_id, step_idx, reason)?;
            Ok(false)
        }
        Err(other) => {
            let _ = broker.mark_https_effect_unknown(
                run.request.run_id,
                step_idx,
                "refresh",
                "refresh_check_unknown",
            );
            Err(other)
        }
    }
}

fn acquire_head_chain(
    broker: &mut PgRunStore,
    run: &RunRecord,
    _step: &HttpsLoopStep,
    step_idx: usize,
) -> Res<()> {
    if !prepare_head_refresh(broker, run, step_idx)? {
        return Ok(());
    }
    let admitted = match broker.reserve_https_head(run.request.run_id, step_idx) {
        Ok(admitted) => admitted,
        Err(Fail::State(reason)) => {
            return broker.block_https_head(run.request.run_id, step_idx, reason);
        }
        Err(other) => {
            let _ = broker.mark_https_effect_unknown(
                run.request.run_id,
                step_idx,
                "head",
                "head_reserve_unknown",
            );
            return Err(other);
        }
    };
    let head_result = fixture_head_result(&admitted);
    broker.commit_https_head(run.request.run_id, step_idx, &head_result)?;
    deliver_https_step(broker, run, step_idx)?;
    broker.close_https_effect_session(run.request.run_id, "completed")
}

fn run_refresh(
    broker: &mut PgRunStore,
    run: &RunRecord,
    step_idx: usize,
    seed_index: usize,
) -> Res<bool> {
    let seed_name = &run.request.seed_names[seed_index].name;
    let issued = match reserve_dns_or_mark_unknown(
        broker,
        run.request.run_id,
        step_idx,
        "refresh",
        seed_index,
    ) {
        Ok(issued) => issued,
        Err(Fail::State(reason)) => {
            broker.block_https_head(run.request.run_id, step_idx, reason)?;
            return Ok(false);
        }
        Err(other) => return Err(other),
    };
    if seed_name == "refreshfail.example.invalid" {
        broker.mark_https_effect_unknown(
            run.request.run_id,
            step_idx,
            "refresh",
            "refresh_failed",
        )?;
        let _ = broker.close_https_effect_session(run.request.run_id, "unknown");
        return Ok(false);
    }
    commit_refresh_dns(broker, run, step_idx, seed_name, issued)
}

fn commit_refresh_dns(
    broker: &mut PgRunStore,
    run: &RunRecord,
    step_idx: usize,
    seed_name: &str,
    issued: TerrainDnsRequest,
) -> Res<bool> {
    let Ok(acquired) = acquire_refresh_request(issued, fixture_dns_answers(seed_name, "refresh"))
    else {
        broker.block_https_head(run.request.run_id, step_idx, "refresh_zero_ttl_or_empty")?;
        let _ = broker.close_https_effect_session(run.request.run_id, "blocked");
        return Ok(false);
    };
    broker.commit_https_dns(run.request.run_id, step_idx, "refresh", &acquired)?;
    let step = broker.https_step(run.request.run_id, step_idx)?;
    let res_id = step
        .refresh_result_id
        .ok_or(Fail::Unresolved("run_terrain_history_pending"))?;
    deliver_dns(run, res_id)?;
    broker.close_https_effect_session(run.request.run_id, "completed")?;
    Ok(true)
}

fn acquire_refresh_request(
    mut req: TerrainDnsRequest,
    answers: Vec<DnsAnswer>,
) -> Res<TerrainDnsRequest> {
    req.answers = answers;
    req.validate()?;
    if req.answers.is_empty() || req.answers.iter().any(|a| a.ttl_seconds == 0) {
        return Err(Fail::State("refresh_zero_ttl_or_empty"));
    }
    Ok(req)
}

pub(super) fn deliver(run: &RunRecord) -> Res<()> {
    let mut broker = PgRunStore::new(m1_reasoning_cli::broker_connection()?);
    for step in broker.https_steps(run.request.run_id)? {
        deliver_step_artifacts(&mut broker, run, &step)?;
    }
    Ok(())
}

fn deliver_step_artifacts(
    broker: &mut PgRunStore,
    run: &RunRecord,
    step: &HttpsLoopStep,
) -> Res<()> {
    for res_id in [step.dns_result_id, step.refresh_result_id]
        .into_iter()
        .flatten()
    {
        deliver_dns(run, res_id)?;
    }
    if step.head_result_id.is_some() {
        let idx = usize::try_from(step.step_index).map_err(|_| Fail::Store("run_decode"))?;
        deliver_https_step(broker, run, idx)?;
    }
    Ok(())
}

fn deliver_dns(run: &RunRecord, result_id: Uuid) -> Res<()> {
    let q = &run.request.context.mission;
    let reference = IssuedDnsReference::controlled_fixture(
        q.engagement_id,
        q.campaign_id,
        result_id,
        &run.request.run_profile,
    )?;
    let mut terrain = PgTerrainStore::new(m1_terrain_cli::terrain_connection()?);
    let mut mission = PgMissionStore::new(connect()?);
    let (event, _, _, _) = terrain
        .record_issued(&mut mission, &reference)?
        .map_err(Fail::State)?;
    publish_dns_history(&event)
}

fn publish_dns_history(event: &duskweave::m1_terrain::TerrainAcceptedEvent) -> Res<()> {
    let history = record_terrain_history(&mut m1_terrain_cli::trajectory_connection()?, event)?;
    (history == HistoryStatus::Completed)
        .then_some(())
        .ok_or(Fail::Unresolved("run_terrain_history_pending"))
}

fn deliver_https_step(broker: &mut PgRunStore, run: &RunRecord, step_idx: usize) -> Res<()> {
    let step = broker.https_step(run.request.run_id, step_idx)?;
    let Some(result) = broker.read_committed_https_result(run.request.run_id, step_idx)? else {
        return Err(Fail::Unresolved("run_terrain_history_pending"));
    };
    let op = OperationId(
        step.head_terrain_operation_id
            .ok_or(Fail::Store("run_decode"))?,
    );
    let mut terrain = PgTerrainStore::new(m1_terrain_cli::terrain_connection()?);
    let mut mission = PgMissionStore::new(connect()?);
    let event = PgHttpsTerrainStore::new(&mut terrain).record_https(&mut mission, &result, op)?;
    publish_https_history(&event)
}

fn publish_https_history(event: &duskweave::m1_https_terrain::TerrainHttpsChangedEvent) -> Res<()> {
    let history =
        record_https_terrain_history(&mut m1_terrain_cli::trajectory_connection()?, event)?;
    (history == HistoryStatus::Completed)
        .then_some(())
        .ok_or(Fail::Unresolved("run_terrain_history_pending"))
}

fn ans(ipv4: &str, ttl_seconds: u32) -> DnsAnswer {
    DnsAnswer {
        ipv4: ipv4.into(),
        ttl_seconds,
    }
}

fn fixture_dns_answers(seed_name: &str, kind: &str) -> Vec<DnsAnswer> {
    if seed_name == "fiveips.example.invalid" {
        return (10..14)
            .map(|n| ans(&format!("198.51.100.{n}"), 60))
            .collect();
    }
    if kind == "initial" {
        initial_dns_answers(seed_name)
    } else {
        refresh_dns_answers(seed_name)
    }
}

fn initial_dns_answers(seed_name: &str) -> Vec<DnsAnswer> {
    let ttl30 = matches!(
        seed_name,
        "ttl30.example.invalid" | "refreshdrop.example.invalid" | "refreshzero.example.invalid"
    );
    if ttl30 {
        vec![ans("198.51.100.10", 30)]
    } else if seed_name == "ttl300.example.invalid" {
        vec![ans("198.51.100.10", 300)]
    } else {
        vec![ans("198.51.100.10", 60), ans("198.51.100.11", 60)]
    }
}

fn refresh_dns_answers(seed_name: &str) -> Vec<DnsAnswer> {
    match seed_name {
        "refreshdrop.example.invalid" => vec![ans("198.51.100.99", 60)],
        "refreshzero.example.invalid" => vec![ans("198.51.100.10", 0)],
        "ttl30.example.invalid" | "ttl300.example.invalid" => vec![ans("198.51.100.10", 60)],
        _ => vec![ans("198.51.100.10", 60), ans("198.51.100.11", 60)],
    }
}

fn fixture_head_result(req: &AdmittedHttpsHeadRequest) -> HttpsHeadResult {
    let now_s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(1_700_000_000))
        .unwrap_or(1_700_000_000);
    let (outcome, status_code, lim) = match (req.hostname.as_str(), req.ipv4.as_str()) {
        ("failboth.example.invalid", _) => (
            HttpsOutcomeKind::ConnectionFailed,
            None,
            "single_endpoint_connection_failed_not_whole_host",
        ),
        ("tlsfail.example.invalid", "198.51.100.10") => (
            HttpsOutcomeKind::TlsFailed,
            None,
            "single_endpoint_tls_failed_not_whole_host",
        ),
        (_, "198.51.100.10") => (
            HttpsOutcomeKind::ConnectionFailed,
            None,
            "single_endpoint_connection_failed_not_whole_host",
        ),
        _ => (
            HttpsOutcomeKind::HeadResponse,
            Some(200),
            "head_response_does_not_prove_access_or_objective_completion",
        ),
    };
    HttpsHeadResult {
        version: VERSION_HEAD_RESULT.into(),
        engagement_id: req.engagement_id,
        campaign_id: req.campaign_id,
        run_id: req.run_id,
        decision_id: req.decision_id,
        attempt_id: req.attempt_id,
        result_id: Uuid::new_v4(),
        asset_ref: req.asset_ref,
        endpoint_ref: req.endpoint_ref.clone(),
        hostname: req.hostname.clone(),
        ipv4: req.ipv4.clone(),
        port: req.port,
        approved_path: req.approved_path.clone(),
        method: req.method.clone(),
        parent_claim_id: req.parent_claim_id,
        parent_claim_revision: req.parent_claim_revision,
        perceived_at: now_s,
        uncertainty_seconds: 0,
        outcome,
        status_code,
        redirect: RedirectDisposition::None,
        source_kind: HttpsSourceKind::FixtureHttpsAcquisition,
        limitations: vec![lim.into()],
    }
}
