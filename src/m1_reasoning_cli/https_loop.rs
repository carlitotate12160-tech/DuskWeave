//! Successor HTTPS-loop native binding and inference composition via existing owners.
use super::{
    PgContextReader, broker_connection, identity, infer_autonomous, prepared_basis, scope, timing,
};
use duskweave::Res;
use duskweave::m1_autonomous_context::AutonomousContext;
use duskweave::m1_context;
use duskweave::m1_context_input::ContextRequest;
use duskweave::m1_reasoning::{
    inference::{Provider, Scope},
    postgres_terrain::TerrainBasis,
    proposal::ProposalBasis,
};
use duskweave::mission::OperationId;
use duskweave::postgres_m1_run::PgRunStore;
use std::time::Instant;
use uuid::Uuid;

pub(crate) fn infer<B, P, G>(
    q: &ContextRequest,
    decision: Uuid,
    run_id: Uuid,
    a: &AutonomousContext,
    basis: impl FnOnce(TerrainBasis) -> B,
    provider: impl FnOnce(&Scope, &m1_context::PreparedContext, Instant) -> Res<(P, G)>,
) -> Res<()>
where
    B: ProposalBasis,
    P: Provider,
{
    let s = scope(q, decision)?;
    let mut o = timing::autonomous_owners(q, &s, a.binding.inference_profile.as_deref())?;
    let prepared = prepare_and_bind(q, decision, run_id, a)?;
    let terrain = prepared_basis(&s, &prepared, o.broker.deadline)?;
    let (provider, _control) = provider(&s, &prepared, o.broker.deadline)?;
    infer_autonomous(q, &s, &mut o, prepared, basis(terrain), provider)
}

fn prepare_and_bind(
    q: &ContextRequest,
    decision: Uuid,
    run_id: Uuid,
    a: &AutonomousContext,
) -> Res<m1_context::PreparedContext> {
    let prep = m1_context::https_loop::prepare(
        &mut PgContextReader::new(broker_connection()?),
        q,
        OperationId(decision),
        run_id,
        a,
    )?;
    bind_and_convert(
        q,
        decision,
        run_id,
        &prep.endpoint_map,
        prep.prepared.utf8().as_bytes(),
    )?;
    Ok(prep.prepared)
}

fn bind_and_convert(
    q: &ContextRequest,
    decision: Uuid,
    run_id: Uuid,
    endpoint_map: &duskweave::m1_https::DecisionEndpointMap,
    pack_utf8: &[u8],
) -> Res<()> {
    let input_hash = identity(q)?;
    let mut run_store = PgRunStore::new(broker_connection()?);
    run_store.bind_https_input(
        q.mission.engagement_id.0,
        q.mission.campaign_id.0,
        decision,
        run_id,
        &input_hash,
        endpoint_map,
        pack_utf8,
    )?;
    run_store.convert_protected_capacity(
        q.mission.engagement_id.0,
        q.mission.campaign_id.0,
        decision,
    )?;
    Ok(())
}
