//! Native CLI composition; inspect/recover have no provider construction or calls.
use duskweave::m1_context;
use duskweave::m1_reasoning::{
    self as reasoning,
    inference::{self, Scope},
    postgres::Basis,
};
use duskweave::mission::OperationId;
use duskweave::postgres_inference_budget::PgInferenceReservationStore;
use duskweave::postgres_mission::{PgAllocator, PgContextReader, PgMissionStore};
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration::OperationAllocator;
use duskweave::{Fail, Res};
use postgres::Client;
use std::time::{Duration, Instant};
use uuid::Uuid;
fn broker_role(c: &mut Client) -> Res<()> {
    qualify(c, "dw_m1_broker")
}
fn cognition_role(c: &mut Client) -> Res<()> {
    qualify(c, "dw_m1_cognition")
}
fn trajectory_role(c: &mut Client) -> Res<()> {
    qualify(c, "dw_m1_trajectory")
}
fn qualify(c: &mut Client, role: &str) -> Res<()> {
    reasoning::postgres::qualify_owner(c, role).map_err(Fail::Config)
}
fn core_connection() -> Res<Client> {
    let mut c = super::connect()?;
    qualify(&mut c, "dw_runtime")?;
    Ok(c)
}
pub(super) fn broker_connection() -> Res<Client> {
    super::database_wait::connect_owner(
        super::database_config::owner_config(
            "DW_INFERENCE_BROKER_DATABASE_URL",
            "DW_INFERENCE_BROKER_DATABASE_URL_FILE",
        )?,
        broker_role,
    )
}
fn cognition_connection() -> Res<Client> {
    super::database_wait::connect_owner(
        super::database_config::owner_config(
            "DW_COGNITION_DATABASE_URL",
            "DW_COGNITION_DATABASE_URL_FILE",
        )?,
        cognition_role,
    )
}
fn history_connection() -> Res<Client> {
    super::database_wait::connect_owner(
        super::database_config::owner_config(
            "DW_TRAJECTORY_DATABASE_URL",
            "DW_TRAJECTORY_DATABASE_URL_FILE",
        )?,
        trajectory_role,
    )
}
fn window(c: &mut Client, s: &Scope) -> Res<Duration> {
    let ms: i64 = c
        .query_one(
            "SELECT execution.inference_window_ms($1,$2,$3)",
            &[&s.engagement, &s.campaign, &s.decision],
        )
        .map_err(|_| Fail::State("inference_authority_unavailable"))?
        .get(0);
    u64::try_from(ms)
        .ok()
        .filter(|v| *v > 0 && *v <= 20000)
        .map(Duration::from_millis)
        .ok_or(Fail::State("inference_deadline"))
}
type ContextRequest = duskweave::m1_context_input::ContextRequest;
pub(super) fn command(command: &str, args: &[String]) -> Res<()> {
    let receipt = command_with(
        command,
        args,
        reasoning::transport::DeepSeekProvider::default,
    )?;
    emit_receipt(&receipt)
}
fn command_with<P: inference::Provider>(
    command: &str,
    args: &[String],
    provider: impl FnOnce() -> P,
) -> Res<inference::Receipt> {
    let (q, operation) = read_input(args)?;
    let fresh = operation == "new";
    if fresh && command != "m1-infer" {
        return Err(Fail::Input("invalid_operation"));
    }
    let s = scope(&q, allocate(&operation)?)?;
    execute(command, q, s, fresh, provider)
}
fn read_input(args: &[String]) -> Res<(ContextRequest, String)> {
    let f = super::flags(args, &["input", "operation"])?;
    let q = super::m1_context_cli::read_request(super::flag(&f, "input")?)?;
    Ok((q, super::flag(&f, "operation")?.to_owned()))
}
fn allocate(operation: &str) -> Res<Uuid> {
    if operation == "new" {
        PgAllocator::new(core_connection()?).allocate()
    } else {
        Uuid::parse_str(operation).map_err(|_| Fail::Input("invalid_operation"))
    }
}
fn scope(q: &ContextRequest, decision: Uuid) -> Res<Scope> {
    if decision.is_nil() {
        return Err(Fail::Input("nil_reference"));
    }
    Ok(Scope {
        engagement: q.mission.engagement_id.0,
        campaign: q.mission.campaign_id.0,
        decision,
        mission_revision: u64::try_from(q.mission.expected_mission_revision)
            .map_err(|_| Fail::Input("inference_input_invalid"))?,
        permission: q.mission.permission_id,
    })
}
fn known_identity(c: &mut Client, s: &Scope, input_hash: &str) -> Res<bool> {
    let known: bool = c.query_one("SELECT EXISTS(SELECT 1 FROM execution.native_input_bindings WHERE e=$1 AND c=$2 AND d=$3) OR EXISTS(SELECT 1 FROM execution.inference_reservations WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3) OR EXISTS(SELECT 1 FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2 AND d=$3)", &[&s.engagement,&s.campaign,&s.decision]).map_err(|_|Fail::Store("inference_inspect_unavailable"))?.get(0);
    if known {
        let same: bool = c.query_one("SELECT EXISTS(SELECT 1 FROM execution.native_input_bindings i WHERE (i.e,i.c,i.d)=($1,$2,$3) AND i.input_hash=$4 AND i.writer_oid=(SELECT oid FROM pg_roles WHERE rolname=session_user)) AND NOT EXISTS(SELECT 1 FROM cognition.reasoning_decisions z WHERE (z.e,z.c,z.d)=($1,$2,$3) AND z.input_hash IS DISTINCT FROM $4)", &[&s.engagement,&s.campaign,&s.decision,&input_hash]).map_err(|_|Fail::Store("inference_identity_unavailable"))?.get(0);
        if !same {
            return Err(Fail::Conflict("inference_identity_conflict"));
        }
    }
    Ok(known)
}
struct Owners {
    broker: reasoning::postgres::PgBroker,
    store: reasoning::postgres::PgDecisionStore,
    history: PgTrajectory,
}
fn owners(
    c: Client,
    q: &ContextRequest,
    deadline: Instant,
    store: reasoning::postgres::PgDecisionStore,
) -> Res<Owners> {
    Ok(Owners {
        broker: reasoning::postgres::PgBroker {
            client: c,
            mission: PgMissionStore::new(core_connection()?),
            reservations: PgInferenceReservationStore::new(broker_connection()?),
            request: q.mission.clone(),
            deadline,
        },
        store,
        history: PgTrajectory::new(history_connection()?),
    })
}
fn execute<P: inference::Provider>(
    command: &str,
    q: ContextRequest,
    s: Scope,
    fresh: bool,
    provider: impl FnOnce() -> P,
) -> Res<inference::Receipt> {
    let (mut c, store) = open_stores(&q)?;
    let known = continuation(&mut c, &s, &store.input_hash, fresh)?;
    let new_inference = command == "m1-infer" && !known;
    let deadline = decision_deadline(&mut c, &s, new_inference)?;
    let mut owners = owners(c, &q, deadline, store)?;
    run(command, new_inference, &q, &s, &mut owners, provider)
}
fn bind_input(c: &mut Client, s: &Scope, hash: &str) -> Res<()> {
    println!(
        "{}",
        serde_json::json!({"kind":"m1_inference_identity_v1","decision":s.decision,"dispatch_granted":false})
    );
    let mut tx = c
        .transaction()
        .map_err(|_| Fail::Store("inference_input_binding_unknown"))?;
    tx.execute(
        "SELECT execution.bind_inference_input($1,$2,$3,$4)",
        &[&s.engagement, &s.campaign, &s.decision, &hash],
    )
    .map_err(|_| Fail::Store("inference_input_binding_unknown"))?;
    tx.commit()
        .map_err(|_| Fail::Store("inference_input_binding_unknown"))?;
    Ok(())
}
fn emit_receipt(receipt: &inference::Receipt) -> Res<()> {
    println!(
        "{}",
        serde_json::to_string(receipt).map_err(|_| Fail::Store("encode"))?
    );
    Ok(())
}
fn open_stores(q: &ContextRequest) -> Res<(Client, reasoning::postgres::PgDecisionStore)> {
    Ok((
        broker_connection()?,
        reasoning::postgres::PgDecisionStore {
            client: cognition_connection()?,
            input_hash: identity(q)?,
        },
    ))
}
fn continuation(c: &mut Client, s: &Scope, input_hash: &str, fresh: bool) -> Res<bool> {
    let known = known_identity(c, s, input_hash)?;
    if !fresh && !known {
        return Err(Fail::State("inference_original_identity_unavailable"));
    }
    if fresh {
        bind_input(c, s, input_hash)?;
    }
    Ok(known)
}
fn decision_deadline(c: &mut Client, s: &Scope, infer: bool) -> Res<Instant> {
    if infer {
        Ok(Instant::now() + window(c, s)?)
    } else {
        Ok(Instant::now())
    }
}
fn run<P: inference::Provider>(
    command: &str,
    infer: bool,
    q: &ContextRequest,
    s: &Scope,
    o: &mut Owners,
    provider: impl FnOnce() -> P,
) -> Res<inference::Receipt> {
    if infer {
        return new_inference(q, s, o, provider());
    }
    // Existing identities never regenerate; recovery constructs no provider.
    inference::inspect_recover(
        s,
        command == "m1-infer-recover",
        &mut o.broker,
        &mut o.store,
        &mut o.history,
    )
    .map_err(Fail::Store)
}
fn new_inference<P: inference::Provider>(
    q: &ContextRequest,
    s: &Scope,
    o: &mut Owners,
    mut provider: P,
) -> Res<inference::Receipt> {
    let p = m1_context::prepare(
        &mut PgContextReader::new(broker_connection()?),
        q,
        OperationId(s.decision),
    )?;
    let mut basis = prepared_basis(s, &p, o.broker.deadline)?;
    let input = inference_input(q, s, p.utf8().as_bytes())?;
    Ok(inference::infer(
        &mut o.broker,
        &mut provider,
        &mut basis,
        &mut o.store,
        &mut o.history,
        input,
    ))
}
fn inference_input<'a>(
    q: &ContextRequest,
    s: &'a Scope,
    pack: &'a [u8],
) -> Res<inference::InferenceInput<'a>> {
    let mode = serde_json::to_value(q.mission.exercise_mode)
        .map_err(|_| Fail::Input("inference_input_invalid"))?;
    Ok(inference::InferenceInput {
        scope: s,
        pack,
        event_id: PgAllocator::new(core_connection()?).allocate()?,
        occurred_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Fail::State("clock_unavailable"))?
            .as_millis() as u64,
        source_mode: mode
            .as_str()
            .ok_or(Fail::Input("inference_input_invalid"))?
            .into(),
    })
}

fn identity(q: &duskweave::m1_context_input::ContextRequest) -> Res<String> {
    use sha2::{Digest, Sha256};
    let b = serde_json::to_vec(q).map_err(|_| Fail::Input("inference_input_invalid"))?;
    Ok(Sha256::digest(b)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect())
}

#[cfg(test)]
#[path = "../tests/support/m1_native_cli.rs"]
mod tests;

pub(super) fn autonomous_inference<B, P, G>(
    q: &ContextRequest,
    decision: Uuid,
    a: &duskweave::m1_autonomous_context::AutonomousContext,
    basis: impl FnOnce(reasoning::postgres_terrain::TerrainBasis) -> B,
    provider: impl FnOnce(&Scope, &m1_context::PreparedContext, Instant) -> Res<(P, G)>,
) -> Res<()>
where
    B: reasoning::proposal::ProposalBasis,
    P: inference::Provider,
{
    let s = scope(q, decision)?;
    let mut o = autonomous_owners(q, &s)?;
    let p = m1_context::prepare_autonomous(
        &mut PgContextReader::new(broker_connection()?),
        q,
        OperationId(decision),
        a,
    )?;
    let terrain = prepared_basis(&s, &p, o.broker.deadline)?;
    let (provider, _control) = provider(&s, &p, o.broker.deadline)?;
    infer_autonomous(q, &s, &mut o, p, basis(terrain), provider)
}
fn infer_autonomous<B: reasoning::proposal::ProposalBasis, P: inference::Provider>(
    q: &ContextRequest,
    s: &Scope,
    o: &mut Owners,
    p: m1_context::PreparedContext,
    mut basis: B,
    mut provider: P,
) -> Res<()> {
    let input = inference_input(q, s, p.utf8().as_bytes())?;
    let receipt = inference::infer(
        &mut o.broker,
        &mut provider,
        &mut basis,
        &mut o.store,
        &mut o.history,
        input,
    );
    let complete = [
        receipt.proposal_disposition == "validated_proposal",
        receipt.publication == "committed",
        receipt.history == "completed",
        receipt.pack_state == "available",
        receipt.consumption == "completed_charged_unused_repair_reserved",
    ];
    complete
        .into_iter()
        .all(|v| v)
        .then_some(())
        .ok_or(Fail::Unresolved("run_inference_pending"))
}
fn autonomous_deadline(c: &mut Client, s: &Scope) -> Res<Instant> {
    let ms: i64 = c
        .query_one(
            "SELECT execution.inference_window_ms($1,$2,$3)",
            &[&s.engagement, &s.campaign, &s.decision],
        )
        .map_err(|_| Fail::State("inference_authority_unavailable"))?
        .try_get(0)
        .map_err(|_| Fail::Store("run_decode"))?;
    let ms = u64::try_from(ms)
        .ok()
        .filter(|v| *v > 0 && *v <= 40000)
        .ok_or(Fail::State("inference_deadline"))?;
    Ok(Instant::now() + Duration::from_millis(ms))
}
pub(super) fn autonomous_recover(q: &ContextRequest, decision: Uuid) -> Res<()> {
    let s = scope(q, decision)?;
    let (c, store) = open_stores(q)?;
    let mut owners = owners(c, q, Instant::now(), store)?;
    inference::inspect_recover(
        &s,
        true,
        &mut owners.broker,
        &mut owners.store,
        &mut owners.history,
    )
    .map_err(Fail::Store)?;
    Ok(())
}

fn autonomous_owners(q: &ContextRequest, s: &Scope) -> Res<Owners> {
    let (mut c, store) = open_stores(q)?;
    if continuation(&mut c, s, &store.input_hash, true)? {
        return Err(Fail::State("run_decision_already_started"));
    }
    let deadline = autonomous_deadline(&mut c, s)?;
    owners(c, q, deadline, store)
}
pub(super) fn prepared_basis(
    s: &Scope,
    p: &m1_context::PreparedContext,
    deadline: Instant,
) -> Res<reasoning::postgres_terrain::TerrainBasis> {
    Ok(reasoning::postgres_terrain::TerrainBasis {
        mission: Basis {
            client: broker_connection()?,
            scope: s.clone(),
            references: reasoning::request::reference_ids(p.utf8().as_bytes())
                .map_err(Fail::Input)?,
            deadline,
        },
        dependencies: p.terrain_dependencies().to_vec(),
    })
}
