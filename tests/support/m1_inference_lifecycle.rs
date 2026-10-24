//! Native-compatible lifecycle fixtures; no inherited consent or provider key.
use crate::{native, run_support};
use duskweave::m1_reasoning::{
    inference::{self, Provider, Scope},
    postgres::{Basis, PgBroker, PgDecisionStore},
    postgres_terrain::TerrainBasis,
    proposal::ProposalBasis,
    request,
};
use duskweave::m1_run::{RunPhase, RunRecord};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

// Classification follows m1_reasoning_cli::autonomous_owners/owners/prepared_basis,
// PgContextReader and PgRunStore. Only closed labels leave PostgreSQL, never SQL.
const READINESS_ACTIVITY: &str = r#"
SELECT coalesce(jsonb_agg(jsonb_build_object(
 'pid',a.pid,'owner',CASE a.usename
   WHEN $1 THEN 'core' WHEN $2 THEN 'broker' WHEN $3 THEN 'cognition'
   WHEN $4 THEN 'trajectory' WHEN $5 THEN 'terrain' ELSE 'other' END,
 'state',CASE WHEN a.state IN ('active','idle','idle in transaction',
   'idle in transaction (aborted)','fastpath function call','disabled')
   THEN a.state ELSE 'unavailable' END,
 'wait_event_type',CASE WHEN a.query IS NULL OR a.query='<insufficient privilege>' THEN 'unavailable'
   WHEN a.wait_event_type IN
   ('Activity','BufferPin','Client','Extension','IO','IPC','Lock','LWLock','Timeout')
   THEN a.wait_event_type WHEN a.wait_event_type IS NULL THEN 'none' ELSE 'other' END,
 'wait_event',CASE WHEN a.query IS NULL OR a.query='<insufficient privilege>' THEN 'unavailable'
   WHEN a.wait_event IN
   ('ClientRead','ClientWrite','DataFileRead','DataFileWrite','WALSync','WALWrite',
    'BufferIO','BufferMapping','transactionid','tuple','relation','advisory')
   THEN a.wait_event WHEN a.wait_event IS NULL THEN 'none' ELSE 'other' END,
 'class',CASE
   WHEN a.query LIKE 'SELECT pg_has_role(session_user,%' THEN 'owner_qualification'
   WHEN a.query LIKE 'SELECT execution.bind_inference_input(%' THEN 'input_binding'
   WHEN a.query LIKE 'SELECT execution.inference_window_ms(%' THEN 'decision_window'
   WHEN a.query LIKE 'SELECT mission.current_inference_publication(%'
     OR a.query LIKE 'SELECT terrain.current_dns_context_view(%' THEN 'context_terrain_read'
   WHEN a.query LIKE 'SELECT execution.run_%' THEN 'run_control_check'
   WHEN a.query LIKE 'SELECT execution.inference_reservation(%' THEN 'reservation'
   ELSE 'other' END) ORDER BY a.pid),'[]'::jsonb),
 count(*) FILTER (WHERE a.query IS NULL OR a.query='<insufficient privilege>')::bigint
FROM pg_stat_activity a
WHERE a.datname=current_database() AND a.pid<>pg_backend_pid()
"#;

const READINESS_STATE: &str = r#"
WITH r AS (SELECT * FROM execution.autonomous_runs WHERE run_id=$1)
SELECT jsonb_build_object('run_known',EXISTS(SELECT 1 FROM r),
 'bindings',(SELECT count(*) FROM execution.native_input_bindings x,r
   WHERE (x.e,x.c)=(r.e,r.c) AND x.d IN(r.decision_one,r.decision_two)),
 'windows',(SELECT count(*) FROM execution.inference_decision_windows x,r
   WHERE (x.e,x.c)=(r.e,r.c) AND x.d IN(r.decision_one,r.decision_two)),
 'reservations',(SELECT count(*) FROM execution.inference_reservations x,r
   WHERE (x.engagement_id,x.campaign_id)=(r.e,r.c) AND x.operation_id IN(r.decision_one,r.decision_two)),
 'calls',(SELECT count(*) FROM execution.inference_calls x,r
   WHERE (x.e,x.c)=(r.e,r.c) AND x.d IN(r.decision_one,r.decision_two)),
 'decisions',(SELECT count(*) FROM cognition.reasoning_decisions x,r
   WHERE (x.e,x.c)=(r.e,r.c) AND x.d IN(r.decision_one,r.decision_two)),
 'history',(SELECT count(*) FROM trajectory.reasoning_history x,r
   WHERE (x.engagement_id,x.campaign_id)=(r.e,r.c) AND x.operation_id IN(r.decision_one,r.decision_two)),
 'effects',(SELECT count(*) FROM execution.fixture_dns_results WHERE run_id=$1),
 'diagnostics',(SELECT coalesce(jsonb_agg(payload ORDER BY phase),'[]')
   FROM execution.run_phase_diagnostics WHERE run_id=$1))
"#;

fn observer_transaction(c: &mut postgres::Client) -> Result<postgres::Transaction<'_>, ()> {
    let mut tx = c
        .build_transaction()
        .read_only(true)
        .start()
        .map_err(|_| ())?;
    tx.batch_execute("SET LOCAL statement_timeout='250ms'; SET LOCAL lock_timeout='50ms'; SET LOCAL idle_in_transaction_session_timeout='1000ms'")
        .map_err(|_| ())?;
    Ok(tx)
}

fn readiness_state(c: &mut postgres::Client, run: Uuid) -> Value {
    let result = (|| {
        let mut tx = observer_transaction(c)?;
        let value: Value = tx
            .query_one(READINESS_STATE, &[&run])
            .map_err(|_| ())?
            .get(0);
        tx.commit().map_err(|_| ())?;
        Ok::<_, ()>(value)
    })();
    result.unwrap_or_else(|_| json!({"status":"unavailable"}))
}

struct ReadinessSamples {
    client: postgres::Client,
    records: Vec<Value>,
    unavailable: bool,
    hidden_samples: u64,
    failed_queries: u64,
    queries: u64,
    query_us: u128,
    max_query_us: u128,
    max_gap_ms: u128,
    envelope_expired: bool,
}

fn sample_readiness(s: &mut ReadinessSamples, roles: &[String], origin: Instant) {
    let started = Instant::now();
    let sample = (|| {
        let mut tx = observer_transaction(&mut s.client)?;
        let row = tx
            .query_one(
                READINESS_ACTIVITY,
                &[&roles[0], &roles[1], &roles[2], &roles[3], &roles[4]],
            )
            .map_err(|_| ())?;
        let records: Value = row.get(0);
        let hidden: i64 = row.get(1);
        tx.commit().map_err(|_| ())?;
        Ok::<_, ()>((records, hidden))
    })();
    let cost = started.elapsed().as_micros();
    s.queries += 1;
    s.query_us += cost;
    s.max_query_us = s.max_query_us.max(cost);
    match sample {
        Ok((mut records, hidden)) => {
            s.unavailable |= hidden > 0;
            s.hidden_samples += u64::from(hidden > 0);
            for record in records.as_array_mut().unwrap() {
                record["elapsed_ms"] = json!(origin.elapsed().as_millis());
            }
            s.records.push(records);
        }
        Err(()) => {
            s.unavailable = true;
            s.failed_queries += 1;
        }
    }
}

/// One preconnected fixture-admin connection, never forwarded to product children.
/// Drop stops and joins sampling even when a CLI/assertion path unwinds.
struct ReadinessObserver {
    run: Uuid,
    origin: Instant,
    launched_ms: u128,
    exited_ms: Option<u128>,
    stop: std::sync::mpsc::Sender<()>,
    worker: Option<std::thread::JoinHandle<ReadinessSamples>>,
}
impl ReadinessObserver {
    fn start(q: &Value) -> Self {
        let origin = Instant::now();
        let run = run_support::id(q);
        let mut client = native::admin();
        let expected = std::env::var("DW_NATIVE_FIXTURE_DATABASE").unwrap();
        let mut tx = observer_transaction(&mut client)
            .unwrap_or_else(|_| panic!("observer_preflight_unavailable"));
        let qualification = tx.query_one(
            "SELECT current_database(), pg_has_role(current_user,'pg_read_all_stats','USAGE') OR (SELECT rolsuper FROM pg_roles WHERE rolname=current_user)", &[]).unwrap();
        let actual: String = qualification.get(0);
        let full_visibility: bool = qualification.get(1);
        tx.commit()
            .unwrap_or_else(|_| panic!("observer_preflight_unavailable"));
        assert_eq!(
            actual, expected,
            "observer must stay in the fresh fixture database"
        );
        println!(
            "readiness_before {}",
            json!({"run":run,"state":readiness_state(&mut client,run)})
        );
        let roles: Vec<String> = [
            "DW_TEST_DATABASE_URL",
            "DW_INFERENCE_BROKER_DATABASE_URL",
            "DW_COGNITION_DATABASE_URL",
            "DW_TRAJECTORY_DATABASE_URL",
            "DW_TERRAIN_DATABASE_URL",
        ]
        .iter()
        .map(|key| {
            let cfg: postgres::Config = std::env::var(key).unwrap().parse().unwrap();
            cfg.get_user().unwrap().to_owned()
        })
        .collect();
        let (stop, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut s = ReadinessSamples {
                client,
                records: Vec::new(),
                // No privilege is granted for diagnostics. Missing visibility is
                // unavailable even if every short-lived backend escaped sampling.
                unavailable: !full_visibility,
                hidden_samples: 0,
                failed_queries: 0,
                queries: 0,
                query_us: 0,
                max_query_us: 0,
                max_gap_ms: 0,
                envelope_expired: false,
            };
            let end = origin + Duration::from_secs(410);
            let mut previous = Instant::now();
            while Instant::now() < end {
                let sampled = Instant::now();
                s.max_gap_ms = s
                    .max_gap_ms
                    .max(sampled.duration_since(previous).as_millis());
                previous = sampled;
                sample_readiness(&mut s, &roles, origin);
                if rx
                    .recv_timeout(
                        (sampled + Duration::from_millis(100))
                            .saturating_duration_since(Instant::now()),
                    )
                    .is_ok()
                {
                    return s;
                }
            }
            s.envelope_expired = true;
            s.unavailable = true;
            s
        });
        Self {
            run,
            origin,
            launched_ms: 0,
            exited_ms: None,
            stop,
            worker: Some(worker),
        }
    }
}
impl Drop for ReadinessObserver {
    fn drop(&mut self) {
        let stopped = Instant::now();
        let _ = self.stop.send(());
        if let Some(Ok(mut s)) = self.worker.take().map(|worker| worker.join()) {
            for sample in s.records {
                println!("readiness_sample {sample}");
            }
            println!(
                "readiness_after {}",
                json!({"run":self.run,
                "state":readiness_state(&mut s.client,self.run)})
            );
            println!(
                "readiness_observer {}",
                json!({"run":self.run,
                "status":if s.unavailable {"unavailable"} else {"observed"},
                "launch_ms":self.launched_ms,"exit_ms":self.exited_ms,
                "queries":s.queries,"query_total_us":s.query_us,"max_query_us":s.max_query_us,
                "hidden_samples":s.hidden_samples,"failed_queries":s.failed_queries,
                "max_sample_gap_ms":s.max_gap_ms,"envelope_expired":s.envelope_expired,
                "stop_join_report_ms":stopped.elapsed().as_millis()})
            );
        } else {
            println!(
                "readiness_observer {}",
                json!({"run":self.run,"status":"unavailable"})
            );
        }
    }
}

pub fn readiness_cli(q: &Value, mut command: std::process::Command) -> std::process::Output {
    let mut observer =
        (std::env::var("DW_TEST_READINESS_OBSERVE").as_deref() == Ok("1")).then(|| {
            match std::panic::catch_unwind(|| ReadinessObserver::start(q)) {
                Ok(observer) => observer,
                Err(failure) => {
                    println!(
                        "readiness_observer {}",
                        json!({"run":run_support::id(q),"status":"unavailable"})
                    );
                    std::panic::resume_unwind(failure)
                }
            }
        });
    if let Some(o) = &mut observer {
        o.launched_ms = o.origin.elapsed().as_millis();
    }
    let output = command.output();
    if let Some(o) = &mut observer {
        o.exited_ms = Some(o.origin.elapsed().as_millis());
    }
    drop(observer);
    output.unwrap_or_else(|_| panic!("lifecycle_cli_launch_unavailable"))
}

pub fn assert_predecessor_catalog() {
    let row = native::admin()
        .query_one(
            "SELECT to_regclass('execution.inference_decision_windows') IS NULL,
         to_regprocedure('execution.successor_configuration(jsonb)') IS NULL
         AND to_regprocedure('execution.assert_successor_pack(jsonb,jsonb,jsonb)') IS NULL,
         to_regclass('execution.run_phase_diagnostics') IS NOT NULL,
         pg_get_constraintdef(oid) FROM pg_constraint
         WHERE conrelid='trajectory.reasoning_history'::regclass
         AND conname='reasoning_history_version_check'",
            &[],
        )
        .unwrap();
    assert!(
        row.get::<_, bool>(0),
        "T8 must start before 0021: successor table already exists"
    );
    assert!(
        row.get::<_, bool>(1),
        "T8 successor function already exists"
    );
    assert!(
        row.get::<_, bool>(2),
        "T8 requires the 0020 diagnostic catalog"
    );
    let constraint: String = row.get(3);
    assert!(
        constraint.contains("version") && constraint.contains("1") && !constraint.contains("2"),
        "T8 requires the predecessor history version constraint"
    );
}

const UPGRADE_TEST: &str =
    "t8_populated_legacy_owner_records_survive_forward_migration_and_repeatable_refresh";
const UPGRADE_MARKER: &str = "DW_TEST_LIFECYCLE_UPGRADE_WORKER";

/// Reenter only this test in a fresh staged fixture using the already-built executable.
pub fn upgrade_worker() -> bool {
    if let Some(marker) = std::env::var_os(UPGRADE_MARKER) {
        assert_eq!(marker, "0020-worker-v1", "invalid private T8 child marker");
        return true;
    }
    let unique = format!(
        "{:x}_{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let database = format!("dw_native_lifecycle_upgrade_{unique}");
    let report = std::env::temp_dir().join(format!("dw-t8-upgrade-{unique}"));
    let python = std::env::var_os("DW_TEST_PYTHON").unwrap_or_else(|| {
        if cfg!(windows) {
            "python".into()
        } else {
            "python3".into()
        }
    });
    let mut command = std::process::Command::new(python);
    command
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env(UPGRADE_MARKER, "0020-worker-v1")
        .env_remove("DEEPSEEK_API_KEY")
        .arg("tests/support/m1_native_bootstrap.py")
        .args([
            "--container",
            if cfg!(windows) { "dwpg-r3" } else { "dwpg" },
        ]);
    if !cfg!(windows) {
        command.arg("--ci");
    }
    command
        .args([
            "--database",
            "fixture",
            "--database-name",
            &database,
            "--schema-revision",
            "0020",
            "--report",
        ])
        .arg(&report)
        .arg("--")
        .arg(std::env::current_exe().unwrap())
        .args([UPGRADE_TEST, "--exact", "--test-threads=1", "--nocapture"]);
    let output = bounded_upgrade_child(command);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "T8 staged fixture failed; report {}: {stdout}\n{stderr}",
        report.display()
    );
    assert!(
        stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored;")
            && stdout.contains("t8_upgrade_worker_complete"),
        "T8 worker must actually execute: {stdout}"
    );
    println!(
        "T8 staged upgrade passed; retained report {}",
        report.display()
    );
    false
}

fn bounded_upgrade_child(mut command: std::process::Command) -> std::process::Output {
    use std::io::Read;
    use std::process::Stdio;
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let deadline = Instant::now() + Duration::from_secs(240);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            // Custodian cleanup after failure is never product termination evidence.
            #[cfg(windows)]
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &child.id().to_string(), "/T", "/F"])
                .output();
            #[cfg(unix)]
            let _ = std::process::Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .output();
            let _ = child.kill();
            let _ = child.wait();
            panic!("T8 staged fixture custodian timeout; qualification failed");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    std::process::Output {
        status,
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
    }
}

/// Read actual immutable owner rows through fresh restricted connections.
pub fn legacy_snapshot() -> Value {
    let owners = [
        (
            "DW_INFERENCE_BROKER_DATABASE_URL",
            &[
                "execution.autonomous_runs",
                "execution.inference_calls",
                "execution.native_input_bindings",
                "execution.inference_reservations",
                "execution.inference_closures",
                "execution.fixture_dns_attempts",
                "execution.fixture_dns_results",
            ][..],
        ),
        (
            "DW_COGNITION_DATABASE_URL",
            &[
                "cognition.reasoning_decisions",
                "cognition.reasoning_outbox",
            ][..],
        ),
        (
            "DW_TRAJECTORY_DATABASE_URL",
            &["trajectory.reasoning_history"][..],
        ),
    ];
    let mut snapshot = serde_json::Map::new();
    for (owner, tables) in owners {
        let mut connection = native::client(owner);
        for table in tables {
            let rows: Value = connection.query_one(&format!(
                "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text),'[]') FROM {table} t"
            ), &[]).unwrap().get(0);
            snapshot.insert((*table).into(), rows);
        }
    }
    Value::Object(snapshot)
}

pub struct Prepared {
    pub scope: Scope,
    pub pack: duskweave::m1_context::PreparedContext,
    pub hash: String,
}
pub fn prepare(rec: &RunRecord, phase: RunPhase) -> Prepared {
    use sha2::{Digest, Sha256};
    let mut run = run_support::broker_store();
    let r = rec.request.run_id;
    run.renew(r).unwrap();
    run.start_phase(r, phase).unwrap();
    run.claim_phase(r, phase, &run_support::release()).unwrap();
    let q = &rec.request.context;
    let d = if phase == RunPhase::DecisionOne {
        rec.decision_one
    } else {
        rec.decision_two
    };
    let hash = Sha256::digest(serde_json::to_vec(q).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let scope = Scope {
        engagement: q.mission.engagement_id.0,
        campaign: q.mission.campaign_id.0,
        decision: d,
        mission_revision: 1,
        permission: q.mission.permission_id,
    };
    native::client("DW_INFERENCE_BROKER_DATABASE_URL")
        .execute(
            "SELECT execution.bind_inference_input($1,$2,$3,$4)",
            &[&scope.engagement, &scope.campaign, &d, &hash],
        )
        .unwrap();
    native::client("DW_INFERENCE_BROKER_DATABASE_URL")
        .query_one(
            "SELECT execution.inference_window_ms($1,$2,$3)",
            &[&scope.engagement, &scope.campaign, &scope.decision],
        )
        .unwrap();
    let a = duskweave::m1_autonomous_context::AutonomousContext {
        binding: duskweave::m1_autonomous_context::RunContextBinding {
            engagement_id: q.mission.engagement_id,
            campaign_id: q.mission.campaign_id,
            run_profile: rec.request.run_profile.clone(),
            execution_mode: rec.request.execution_mode.clone(),
            inference_profile: rec.request.inference_profile.clone(),
            release_sha256: rec.release_sha256.clone(),
            qualification_ref: rec.qualification_ref,
        },
        predecessor: if phase == RunPhase::DecisionTwo {
            Some(run.predecessor(r).unwrap())
        } else {
            None
        },
    };
    let pack = duskweave::m1_context::prepare_autonomous(
        &mut duskweave::postgres_mission::PgContextReader::new(native::client(
            "DW_INFERENCE_BROKER_DATABASE_URL",
        )),
        q,
        duskweave::mission::OperationId(d),
        &a,
    )
    .unwrap();
    Prepared { scope, pack, hash }
}

pub struct OwnerBasis(pub TerrainBasis);
impl ProposalBasis for OwnerBasis {
    fn reference_eligible(&self, r: &str) -> bool {
        self.0.reference_eligible(r)
    }
    fn manifest_eligible(&self, r: &str) -> bool {
        r == "dns_udp"
    }
    fn safe_explanation(&self, s: &str) -> bool {
        self.0.safe_explanation(s)
    }
    fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
        self.0.current_authority_and_premises()
    }
    fn before_provider_release(&mut self) -> Result<(), &'static str> {
        self.0.before_provider_release()
    }
    fn current_authority_for_references(&mut self, r: &[String]) -> Result<(), &'static str> {
        self.0.recording_references(r)
    }
    fn recording_premises(
        &mut self,
    ) -> Result<duskweave::m1_reasoning::proposal::RecordingPremises, &'static str> {
        self.0.recording_integrity()
    }
}
/// Consumer qualification injects the typed owner result; real TTL classification
/// is exercised separately against the issued Terrain lineage and original times.
pub struct ExpiredAtRecording(pub OwnerBasis);
impl ProposalBasis for ExpiredAtRecording {
    fn reference_eligible(&self, r: &str) -> bool {
        self.0.reference_eligible(r)
    }
    fn manifest_eligible(&self, r: &str) -> bool {
        self.0.manifest_eligible(r)
    }
    fn safe_explanation(&self, s: &str) -> bool {
        self.0.safe_explanation(s)
    }
    fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
        self.0.current_authority_and_premises()
    }
    fn before_provider_release(&mut self) -> Result<(), &'static str> {
        self.0.before_provider_release()
    }
    fn current_authority_for_references(&mut self, r: &[String]) -> Result<(), &'static str> {
        self.0.current_authority_for_references(r)
    }
    fn recording_premises(
        &mut self,
    ) -> Result<duskweave::m1_reasoning::proposal::RecordingPremises, &'static str> {
        self.0.recording_premises()?;
        Ok(duskweave::m1_reasoning::proposal::RecordingPremises::ExpiredByTtl)
    }
}
pub fn broker(rec: &RunRecord, prepared: &Prepared) -> PgBroker {
    let origin = Instant::now();
    let mut client = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let left: i64 = client
        .query_one(
            "SELECT execution.inference_window_ms($1,$2,$3)",
            &[
                &prepared.scope.engagement,
                &prepared.scope.campaign,
                &prepared.scope.decision,
            ],
        )
        .unwrap()
        .get(0);
    PgBroker {
        client,
        mission: duskweave::postgres_mission::PgMissionStore::new(native::client(
            "DW_TEST_DATABASE_URL",
        )),
        reservations: duskweave::postgres_inference_budget::PgInferenceReservationStore::new(
            native::client("DW_INFERENCE_BROKER_DATABASE_URL"),
        ),
        request: rec.request.context.mission.clone(),
        deadline: origin + Duration::from_millis(left as u64),
    }
}
pub fn basis(prepared: &Prepared, deadline: Instant) -> OwnerBasis {
    OwnerBasis(TerrainBasis {
        mission: Basis {
            client: native::client("DW_INFERENCE_BROKER_DATABASE_URL"),
            scope: prepared.scope.clone(),
            references: request::reference_ids(prepared.pack.utf8().as_bytes()).unwrap(),
            deadline,
        },
        dependencies: prepared.pack.terrain_dependencies().to_vec(),
    })
}
pub fn infer(
    rec: &RunRecord,
    prepared: &Prepared,
    provider: &mut impl Provider,
) -> inference::Receipt {
    let mut broker = broker(rec, prepared);
    let mut basis = basis(prepared, broker.deadline);
    inference::infer(
        &mut broker,
        provider,
        &mut basis,
        &mut PgDecisionStore {
            client: native::client("DW_COGNITION_DATABASE_URL"),
            input_hash: prepared.hash.clone(),
        },
        &mut duskweave::postgres_trajectory::PgTrajectory::new(native::client(
            "DW_TRAJECTORY_DATABASE_URL",
        )),
        inference::InferenceInput {
            scope: &prepared.scope,
            pack: prepared.pack.utf8().as_bytes(),
            event_id: run_support::consent_id(),
            occurred_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            source_mode: "blind".into(),
        },
    )
}
pub struct DelayedFixture {
    pub inner: duskweave::m1_reasoning::fixture::ProtocolFixture,
    pub before_release: bool,
    pub entries: usize,
    pub after_release: Option<Box<dyn FnOnce()>>,
}

/// Private encoded-response construction; all output still crosses the selected real codec.
pub struct WireFixture {
    pub hypothesis: String,
    pub fault: &'static str,
    pub entries: usize,
}
pub struct FaultStore {
    pub inner: PgDecisionStore,
    pub fail: bool,
}
impl inference::DecisionStore for FaultStore {
    fn publish_atomic(&mut self, event: &inference::Decision) -> Result<(), &'static str> {
        if self.fail {
            return Err("inference_publication_pending");
        }
        self.inner.publish_atomic(event)
    }
    fn inspect(&mut self, scope: &Scope) -> Result<Option<inference::Decision>, &'static str> {
        self.inner.inspect(scope)
    }
}
pub struct FaultHistory {
    pub inner: duskweave::postgres_trajectory::PgTrajectory,
    pub fail: bool,
}
impl inference::ReasoningHistory for FaultHistory {
    fn accept_committed_source(
        &mut self,
        event: &inference::Decision,
    ) -> Result<&'static str, &'static str> {
        if self.fail {
            return Err("history_pending");
        }
        self.inner.accept_committed_source(event)
    }
    fn inspect(&mut self, event: &inference::Decision) -> Result<&'static str, &'static str> {
        self.inner.inspect(event)
    }
}
impl Provider for WireFixture {
    fn prepare(&mut self, b: &request::BoundCall, _: Instant) -> Result<(), &'static str> {
        request::verify_binding(
            std::str::from_utf8(&b.pack).unwrap(),
            std::str::from_utf8(&b.configuration).unwrap(),
            std::str::from_utf8(&b.body).unwrap(),
            &b.digest,
            self.entries != 0,
        )
    }
    fn call(
        &mut self,
        b: &request::BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<inference::Received, &'static str> {
        release()?;
        self.entries += 1;
        if self.fault == "provider" {
            return Err("provider_transport_unknown");
        }
        let proposal = if self.fault == "repair" && self.entries == 1 {
            json!({"incomplete":"shape"})
        } else {
            json!({"version":"m1_inference_proposal_v1","hypothesis":self.hypothesis,
                "considered_alternative":"Routing remains unavailable.","references":["asset_0"],
                "expected_observation":"Fixture routing metadata.","falsifying_observation":"No qualified current premise.",
                "uncertainty":"Synthetic evidence only.","rationale":"A proposal grants no dispatch.",
                "next_intent":{"category":"observe_more","references":["asset_0"],"manifest_reference":"dns_udp","view_request":null}})
        };
        let chat = serde_json::from_slice::<Value>(&b.configuration).unwrap()["protocol"]
            == "chat-completions-v1";
        let envelope = if chat {
            json!({"choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":proposal.to_string()}}]})
        } else {
            json!({"status":"completed","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":proposal.to_string()}]}]})
        };
        let raw = serde_json::to_vec(&envelope).unwrap();
        let mut received = if chat {
            duskweave::m1_reasoning::chat_completions::admit_envelope(&raw, Duration::ZERO, basis)?
        } else {
            duskweave::m1_reasoning::response::admit_envelope(
                &raw,
                raw.len(),
                Duration::ZERO,
                basis,
            )?
        };
        received.usage.requested_model = "protocol-fixture-v1";
        Ok(received)
    }
}

pub fn eight_seeds(raw: &mut Value) {
    let rule = raw["m1_permission"]["contact_rules"][0].clone();
    let assets: Vec<_> = (0..8)
        .map(|i| Uuid::from_u128(if i == 0 { 0x21 } else { 0x100 + i }))
        .collect();
    raw["included_assets"] = json!(assets);
    raw["m1_permission"]["contact_rules"] = json!(
        assets
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let mut rule = rule.clone();
                rule["asset_ref"] = json!(id);
                rule["priority"] = json!(i + 1);
                if i > 0 {
                    rule["rule"] = json!({"exact":format!("seed{i}.example.invalid")});
                }
                rule
            })
            .collect::<Vec<_>>()
    );
}

pub fn wire_case(name: &str, pack: &str) -> Value {
    let generation = request::bind(pack.as_bytes(), None).unwrap();
    let repair = request::bind(pack.as_bytes(), Some(&["unexpected_view_request"; 8])).unwrap();
    for (bound, is_repair) in [(&generation, false), (&repair, true)] {
        request::verify_binding(
            pack,
            std::str::from_utf8(&bound.configuration).unwrap(),
            std::str::from_utf8(&bound.body).unwrap(),
            &bound.digest,
            is_repair,
        )
        .unwrap();
    }
    println!(
        "wire {name}: P={} C={} generation={} repair={} margins={}/{}/{}",
        pack.len(),
        generation.configuration.len(),
        generation.body.len(),
        repair.body.len(),
        8192 - pack.len(),
        16384 - generation.configuration.len(),
        16384 - repair.body.len()
    );
    json!({"name":name,"pack":pack,"configuration":std::str::from_utf8(&generation.configuration).unwrap(),
        "generation_request":std::str::from_utf8(&generation.body).unwrap(),"generation_binding":generation.digest,
        "repair_request":std::str::from_utf8(&repair.body).unwrap(),"repair_binding":repair.digest,
        "utf8_bytes":{"pack":pack.len(),"configuration":generation.configuration.len(),"generation":generation.body.len(),"repair":repair.body.len()}})
}
pub fn restricted_wire_checks(rec: &RunRecord, p: &Prepared) {
    use inference::Broker;
    let mut broker = broker(rec, p);
    let reservation = broker.reserve(&p.scope, p.pack.utf8().as_bytes()).unwrap();
    let bound = request::bind(p.pack.utf8().as_bytes(), None).unwrap();
    let scope = &p.scope;
    let mut client = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let invoke = |tx: &mut postgres::Transaction<'_>, b: &request::BoundCall, call: Uuid| {
        tx.query_one(
            "SELECT execution.inference_may_send($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &scope.engagement,
                &scope.campaign,
                &scope.decision,
                &call,
                &std::str::from_utf8(&b.pack).unwrap(),
                &std::str::from_utf8(&b.configuration).unwrap(),
                &std::str::from_utf8(&b.body).unwrap(),
                &b.digest,
            ],
        )
    };
    for field in [
        "permission_id",
        "mission_revision",
        "goal_ref",
        "scope",
        "question",
    ] {
        let mut pack: Value = serde_json::from_slice(&bound.pack).unwrap();
        pack[field] = if field == "mission_revision" {
            json!(2)
        } else {
            Value::Null
        };
        let forged = request::bind(&serde_json::to_vec(&pack).unwrap(), None).unwrap();
        let mut tx = client.transaction().unwrap();
        assert!(
            invoke(&mut tx, &forged, reservation.generation_call).is_err(),
            "SQL accepted forged pack {field}"
        );
        tx.rollback().unwrap();
    }
    for key in [
        "profile",
        "recipient",
        "protocol",
        "credential_reference",
        "requested_settings",
        "limits",
        "schema",
        "transport",
    ] {
        let mut config: Value = serde_json::from_slice(&bound.configuration).unwrap();
        config[key] = if config[key].is_null() {
            json!("substituted")
        } else {
            Value::Null
        };
        let cfg = serde_json::to_vec(&config).unwrap();
        let forged = request::BoundCall {
            pack: bound.pack.clone(),
            digest: request::compute_binding(&bound.pack, &cfg, &bound.body),
            configuration: cfg,
            body: bound.body.clone(),
        };
        let mut tx = client.transaction().unwrap();
        assert!(
            invoke(&mut tx, &forged, reservation.generation_call).is_err(),
            "SQL accepted forged {key}"
        );
        tx.rollback().unwrap();
    }
    let repair = request::bind(&bound.pack, Some(&["unexpected_view_request"; 8])).unwrap();
    let mut tx = client.transaction().unwrap();
    assert!(
        invoke(&mut tx, &repair, reservation.repair_call).is_err(),
        "repair requires completed original generation"
    );
    tx.rollback().unwrap();
    let mut tx = client.transaction().unwrap();
    invoke(&mut tx, &bound, reservation.generation_call).unwrap();
    tx.query_one(
        "SELECT execution.inference_call_state($1,'completed',$2)",
        &[
            &reservation.generation_call,
            &json!({"input_tokens":null,"output_tokens":null}),
        ],
    )
    .unwrap();
    invoke(&mut tx, &repair, reservation.repair_call).unwrap();
    tx.rollback().unwrap();
    // Wire admission is rolled back; this binding-only case publishes no decision.
}
impl Provider for DelayedFixture {
    fn prepare(&mut self, b: &request::BoundCall, end: Instant) -> Result<(), &'static str> {
        self.inner.prepare(b, end)?;
        if self.before_release {
            std::thread::sleep(Duration::from_secs(5));
        }
        Ok(())
    }
    fn call(
        &mut self,
        b: &request::BoundCall,
        end: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<inference::Received, &'static str> {
        self.entries += 1;
        self.inner.call(b, end, basis, &mut || {
            release()?;
            if let Some(after) = self.after_release.take() {
                after();
            }
            Ok(())
        })
    }
}

pub fn seed_claim(rec: &RunRecord, ttl: u32) -> Value {
    let profile =
        duskweave::m1_inference_profile::lookup(rec.request.inference_profile.as_deref().unwrap())
            .unwrap();
    seed_claim_with(
        rec,
        ttl,
        &mut duskweave::m1_reasoning::fixture::ProtocolFixture::new(profile).unwrap(),
    )
}
pub fn seed_claim_with(rec: &RunRecord, ttl: u32, provider: &mut impl Provider) -> Value {
    let first = prepare(rec, RunPhase::DecisionOne);
    let receipt = infer(rec, &first, provider);
    assert_eq!(receipt.history, "completed");
    let mut run = run_support::broker_store();
    let r = rec.request.run_id;
    run.complete_phase(r, RunPhase::DecisionOne).unwrap();
    run.start_phase(r, RunPhase::Acquisition).unwrap();
    run.claim_phase(r, RunPhase::Acquisition, &run_support::release())
        .unwrap();
    let result = duskweave::m1_fixture_dns::acquire(run.reserve_effect(r, 0).unwrap()).unwrap();
    run.commit_result(r, &result).unwrap();
    let m = &rec.request.context.mission;
    let reference =
        serde_json::from_value(json!({"version":"m1_terrain_dns_issued_observation_v1",
        "engagement_id":m.engagement_id,"campaign_id":m.campaign_id,"result_ref":rec.result,
        "run_profile":rec.request.run_profile,"execution_mode":rec.request.execution_mode}))
        .unwrap();
    let event = duskweave::postgres_m1_terrain::PgTerrainStore::new(native::client(
        "DW_TERRAIN_DATABASE_URL",
    ))
    .record_issued(
        &mut duskweave::postgres_mission::PgMissionStore::new(native::client(
            "DW_TEST_DATABASE_URL",
        )),
        &reference,
    )
    .unwrap()
    .unwrap()
    .0;
    duskweave::postgres_terrain_history::record_terrain_history(
        &mut native::client("DW_TRAJECTORY_DATABASE_URL"),
        &event,
    )
    .unwrap();
    run.complete_phase(r, RunPhase::Acquisition).unwrap();
    let original = serde_json::to_value(result).unwrap();
    // Preserve the actual issued 60s TTL and wait until only the selected remainder is fresh.
    let ready_at = original["perceived_at"].as_i64().unwrap() + 60 - i64::from(ttl);
    loop {
        let now: i64 = native::admin()
            .query_one(
                "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
                &[],
            )
            .unwrap()
            .get(0);
        if now >= ready_at {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    original
}

pub fn registered(slot: u128, profile: &str) -> Value {
    registered_with(slot, profile, |_| {})
}
pub fn registered_with(slot: u128, profile: &str, mutate: impl FnOnce(&mut Value)) -> Value {
    let selected = duskweave::m1_inference_profile::lookup(profile).unwrap();
    let mut q = run_support::registered_with(slot, |raw| {
        let p = &mut raw["m1_inference_permission"];
        p["version"] = json!("m1_inference_permission_v4");
        p["profile"] = json!(selected.name);
        p["recipient"] = json!(selected.connection.recipient);
        p["run_profile"] = json!("autonomous-dns-fixture-v2");
        p["campaign_limits"]["wall_time_ms"] = json!(258000);
        for (key, value) in [
            ("soft_completion_ms", 60000),
            ("per_call_wall_time_ms", 62000),
            ("shared_wall_time_ms", 124000),
            ("decision_wall_time_ms", 129000),
            ("phase_wall_time_ms", 134000),
        ] {
            p["operation_limits"][key] = json!(value);
        }
        mutate(raw);
    });
    q["version"] = json!("m1_autonomous_run_request_v2");
    q["inference_profile"] = json!(selected.name);
    let now: i64 = native::admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    q["deadline_at_ms"] = json!(now + 399000);
    q
}

pub fn assert_bound_calls(q: &Value, profile: &str) {
    let r = run_support::id(q);
    let rows = native::admin().query("SELECT p,configuration,body,binding,usage FROM execution.inference_calls WHERE d IN (SELECT decision_one FROM execution.autonomous_runs WHERE run_id=$1 UNION ALL SELECT decision_two FROM execution.autonomous_runs WHERE run_id=$1)", &[&r]).unwrap();
    assert_eq!(rows.len(), 2);
    let mut feedback = 0;
    for row in rows {
        let p: String = row.get(0);
        let c: String = row.get(1);
        let b: String = row.get(2);
        let h: String = row.get(3);
        duskweave::m1_reasoning::request::verify_binding(&p, &c, &b, &h, false).unwrap();
        let pack: Value = serde_json::from_str(&p).unwrap();
        let config: Value = serde_json::from_str(&c).unwrap();
        let body: Value = serde_json::from_str(&b).unwrap();
        assert_eq!(pack["version"], "m1_context_pack_autonomous_v2");
        assert_eq!(pack["inference_profile"], profile);
        assert_eq!(config["profile"], profile);
        assert_eq!(config["timing_profile"], "reasoning-bounded-v1");
        let usage: Value = row.get(4);
        assert_eq!(usage["requested_model"], "protocol-fixture-v1");
        assert_eq!(usage["reported_model"], "protocol-fixture-v1");
        if profile == "fixture-chat-completions-v1" {
            assert_eq!(body["messages"][1]["content"], p);
            assert!(body.get("input").is_none());
        } else {
            assert_eq!(body["input"], p);
        }
        if pack["predecessor"]["availability"] == "committed" {
            feedback += 1;
        }
    }
    assert_eq!(feedback, 1);
    let _: Uuid = r;
}
