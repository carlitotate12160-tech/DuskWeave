//! Foreground run composition. Only fixed binary child phases execute the run.
use duskweave::m1_run::{NextStep, RunPhase, RunRecord, RunRequest};
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::{Fail, Res};
use std::io::Read;
use std::time::{Duration, Instant};
use uuid::Uuid;

fn store() -> Res<PgRunStore> {
    Ok(PgRunStore::new(
        super::m1_reasoning_cli::broker_connection()?
    ))
}
pub(super) fn command(command: &str, args: &[String]) -> Res<()> {
    match command {
        "m1-run" => start(args),
        "m1-run-phase" => child(args),
        "m1-run-inspect" | "m1-run-recover" | "m1-run-stop" => inspect(command, args),
        _ => Err(Fail::Input("unknown_command")),
    }
}
fn read_request(args: &[String]) -> Res<RunRequest> {
    let flags = super::flags(args, &["input"])?;
    let file = std::fs::File::open(super::flag(&flags, "input")?)
        .map_err(|_| Fail::Input("unreadable_input"))?;
    let mut bytes = Vec::new();
    file.take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| Fail::Input("unreadable_input"))?;
    RunRequest::parse(&bytes)
}

fn start(args: &[String]) -> Res<()> {
    let q = read_request(args)?;
    let mut store = store()?;
    let outcome = start_run(&mut store, &q, &release()?);
    let inspected = emit(&mut store, q.run_id);
    outcome.and(inspected)
}
fn start_run(store: &mut PgRunStore, q: &RunRequest, release: &str) -> Res<()> {
    let first = store.start(q, release)?;
    let record = store.inspect(q.run_id)?;
    if first {
        supervise(store, &record)?;
    }
    Ok(())
}
fn supervise(store: &mut PgRunStore, run: &RunRecord) -> Res<()> {
    let deadline = run_deadline(run)?;
    if let NextStep::Terminal(reason) = first_decision(store, run, deadline)? {
        return store.terminal(run.request.run_id, reason);
    }
    for phase in [RunPhase::Acquisition, RunPhase::DecisionTwo] {
        drive_phase(store, run, phase, deadline)?;
    }
    Ok(())
}
fn first_decision(store: &mut PgRunStore, run: &RunRecord, deadline: Instant) -> Res<NextStep> {
    drive_phase(store, run, RunPhase::DecisionOne, deadline)?;
    let proposal = store.first_proposal(run)?;
    duskweave::m1_run::next_step(&proposal, run.request.seed_names.len())
}
fn drive_phase(
    store: &mut PgRunStore,
    run: &RunRecord,
    phase: RunPhase,
    run_deadline: Instant,
) -> Res<()> {
    let r = run.request.run_id;
    store.renew(r)?;
    store.start_phase(r, phase)?;
    let deadline = super::m1_run_supervision::phase_deadline(run_deadline, phase);
    let outcome = wait_phase(store, run, phase, deadline);
    if outcome.is_err() {
        let _ = store.unknown(r, phase);
    }
    outcome
}
fn wait_phase(
    store: &mut PgRunStore,
    run: &RunRecord,
    phase: RunPhase,
    deadline: Instant,
) -> Res<()> {
    let r = run.request.run_id;
    let child = super::m1_run_supervision::PhaseChild::launch(run, phase, deadline)?;
    let mut renewal = Instant::now();
    while !child.finished()? {
        if Instant::now() >= deadline {
            return Err(Fail::Unresolved("run_child_deadline"));
        }
        if Instant::now() >= renewal {
            store.renew(r)?;
            renewal = Instant::now() + Duration::from_millis(400);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    store.complete_phase(r, phase)
}
fn run_deadline(run: &RunRecord) -> Res<Instant> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Fail::State("clock_unavailable"))?
        .as_millis();
    let remain = u128::try_from(run.deadline_at_ms)
        .ok()
        .and_then(|d| d.checked_sub(now))
        .filter(|d| *d <= 150000)
        .ok_or(Fail::State("run_deadline"))?;
    Ok(Instant::now() + Duration::from_millis(remain as u64))
}
fn parse_run(raw: &str) -> Res<Uuid> {
    Uuid::parse_str(raw)
        .ok()
        .filter(|r| !r.is_nil())
        .ok_or(Fail::Input("invalid_args"))
}
fn child_args(args: &[String]) -> Res<(Uuid, RunPhase)> {
    let flags = super::flags(args, &["run", "phase"])?;
    let r = parse_run(super::flag(&flags, "run")?)?;
    let phase = RunPhase::parse(super::flag(&flags, "phase")?)?;
    Ok((r, phase))
}
fn claim_child(
    args: &[String],
) -> Res<(
    RunRecord,
    RunPhase,
    Option<super::m1_run_control::Preparation>,
)> {
    let (r, phase) = child_args(args)?;
    let mut store = store()?;
    let record = store.claim_phase(r, phase, &release()?)?;
    let preparation = (phase != RunPhase::Acquisition
        && record.request.run_profile == duskweave::m1_run::PROVIDER_FIXTURE_PROFILE)
        .then(super::m1_run_control::Preparation::start)
        .transpose()?;
    Ok((record, phase, preparation))
}
fn child(args: &[String]) -> Res<()> {
    let (record, phase, preparation) = claim_child(args)?;
    match phase {
        RunPhase::Acquisition => super::m1_run_effect::acquire(&record),
        _ => super::m1_run_inference::decide(&record, phase, preparation),
    }
}
fn inspect(command: &str, args: &[String]) -> Res<()> {
    let r = run_reference(args)?;
    let control = if command == "m1-run-stop" {
        super::m1_run_control::signal_run(r)
    } else {
        Ok(())
    };
    let mut store = store()?;
    let authoritative = match command {
        "m1-run-stop" => store.stop(r),
        "m1-run-recover" => recover(&mut store, r),
        _ => Ok(()),
    };
    let receipt = emit(&mut store, r);
    authoritative.and(control).and(receipt)
}
fn recover(store: &mut PgRunStore, r: Uuid) -> Res<()> {
    let run = store.inspect(r)?;
    // Exactly three bounded DB-only delivery attempts. No phase launch, provider, repair or acquisition.
    for _ in 0..3 {
        let decisions = super::m1_run_inference::recover(&run);
        let terrain = super::m1_run_effect::deliver(&run);
        if decisions.is_ok() && terrain.is_ok() {
            break;
        }
    }
    Ok(())
}
fn emit(store: &mut PgRunStore, r: Uuid) -> Res<()> {
    println!("{}", store.summary(r)?);
    Ok(())
}
fn release() -> Res<String> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(
        std::env::current_exe().map_err(|_| Fail::Config("run_binary_unavailable"))?,
    )
    .map_err(|_| Fail::Config("run_binary_unavailable"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|_| Fail::Config("run_binary_unavailable"))?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(hash.finalize().iter().map(|v| format!("{v:02x}")).collect())
}

fn run_reference(args: &[String]) -> Res<Uuid> {
    let flags = super::flags(args, &["run"])?;
    parse_run(super::flag(&flags, "run")?)
}
