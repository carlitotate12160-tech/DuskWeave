//! Physical PostgreSQL COMMIT ACK loss on the real native application ports.
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_native_inference.rs"]
mod native;
#[path = "support/database_wait_proxy.rs"]
mod proxy;
use duskweave::m1_reasoning::{
    inference::*, postgres::Basis, proposal::ProposalBasis, request, response,
};
use duskweave::registration::OperationAllocator;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

struct ProviderFixture(Arc<AtomicUsize>);
impl Provider for ProviderFixture {
    fn prepare(&mut self, _: &request::BoundCall, _: Instant) -> Result<(), &'static str> {
        Ok(())
    }
    fn call(
        &mut self,
        b: &request::BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        release()?;
        self.0.fetch_add(1, Ordering::SeqCst);
        if std::env::var_os("DW_NATIVE_PROCESS_EXIT").is_some() {
            use std::io::Write;
            println!("native_exit_released");
            std::io::stdout().flush().unwrap();
            loop {
                std::thread::park();
            }
        }
        let refs = request::reference_ids(&b.pack)?;
        let proposal = json!({"version":"m1_inference_proposal_v1","hypothesis":"bounded hypothesis",
            "considered_alternative":"alternative","references":[refs[0]],"expected_observation":"expected",
            "falsifying_observation":"falsifying","uncertainty":"unknown","rationale":"bounded explanation",
            "next_intent":{"category":"observe_more","references":[refs[0]],"manifest_reference":null,"view_request":null}});
        let envelope = json!({"status":"completed","model":"deepseek-flash","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":proposal.to_string()}]}],"usage":{"input_tokens":10,"output_tokens":20}});
        response::admit_envelope(
            &serde_json::to_vec(&envelope).unwrap(),
            100,
            Duration::ZERO,
            basis,
        )
    }
}

fn count(table: &str, s: &Scope) -> i64 {
    let (e, c, d) =
        if table == "trajectory.reasoning_history" || table == "execution.inference_reservations" {
            ("engagement_id", "campaign_id", "operation_id")
        } else {
            ("e", "c", "d")
        };
    native::admin()
        .query_one(
            &format!("SELECT count(*) FROM {table} WHERE {e}=$1 AND {c}=$2 AND {d}=$3"),
            &[&s.engagement, &s.campaign, &s.decision],
        )
        .unwrap()
        .get(0)
}

fn recover(request: &Value, s: &Scope) -> Value {
    let path = std::env::temp_dir().join(format!("dw-native-ack-{}.json", s.decision));
    std::fs::write(&path, serde_json::to_vec(request).unwrap()).unwrap();
    let mut last = Value::Null;
    for command in ["m1-infer-inspect", "m1-infer-recover", "m1-infer-recover"] {
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_duskweave"));
        child
            .env_clear()
            .env("DW_DATABASE_CONFIG_MODE", "env-local")
            .env(
                "DW_DATABASE_URL",
                std::env::var("DW_TEST_DATABASE_URL").unwrap(),
            );
        for variable in [
            "DW_INFERENCE_BROKER_DATABASE_URL",
            "DW_COGNITION_DATABASE_URL",
            "DW_TRAJECTORY_DATABASE_URL",
            "SystemRoot",
            "LLVM_PROFILE_FILE",
        ] {
            if let Some(value) = std::env::var_os(variable) {
                child.env(variable, value);
            }
        }
        let result = child
            .arg(command)
            .arg("--input")
            .arg(&path)
            .arg("--operation")
            .arg(s.decision.to_string())
            .output()
            .unwrap();
        assert!(
            result.status.success() && result.stderr.is_empty(),
            "fresh native recovery succeeds"
        );
        let receipt: Value = serde_json::from_slice(
            result
                .stdout
                .split(|b| *b == b'\n')
                .find(|line| line.first() == Some(&b'{'))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(receipt["decision"], s.decision.to_string());
        assert_eq!(receipt["recovery_category"], "no_provider_replay");
        assert_eq!(receipt["dispatch_granted"], false);
        last = receipt;
    }
    std::fs::remove_file(path).unwrap();
    last
}

#[test]
fn committed_reservation_marker_publication_and_history_survive_lost_ack() {
    let _guard = native::db();
    for (index, phase) in ["reservation", "may_send", "publication", "history"]
        .into_iter()
        .enumerate()
    {
        let mut case = native::decision(0x9900 + index as u128);
        let s = case.scope.clone();
        let q = case.request.clone();
        let relay = proxy::Relay::start("127.0.0.1:5432".parse().unwrap(), b"COMMIT");
        match phase {
            "reservation" => {
                case.broker.reservations =
                    duskweave::postgres_inference_budget::PgInferenceReservationStore::new(
                        native::through("DW_INFERENCE_BROKER_DATABASE_URL", relay.port()),
                    )
            }
            "may_send" => {
                case.broker.client =
                    native::through("DW_INFERENCE_BROKER_DATABASE_URL", relay.port())
            }
            "publication" => {
                case.store.client = native::through("DW_COGNITION_DATABASE_URL", relay.port())
            }
            "history" => {
                case.history = duskweave::postgres_trajectory::PgTrajectory::new(native::through(
                    "DW_TRAJECTORY_DATABASE_URL",
                    relay.port(),
                ))
            }
            _ => unreachable!(),
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let receipt = std::thread::scope(|threads| {
            let worker = threads.spawn(move || {
                let event = duskweave::postgres_mission::PgAllocator::new(native::client(
                    "DW_TEST_DATABASE_URL",
                ))
                .allocate()
                .unwrap();
                let mut basis = Basis {
                    client: native::client("DW_INFERENCE_BROKER_DATABASE_URL"),
                    scope: case.scope.clone(),
                    references: request::reference_ids(&case.pack).unwrap(),
                    deadline: case.broker.deadline,
                };
                duskweave::m1_reasoning::inference::infer(
                    &mut case.broker,
                    &mut ProviderFixture(observed),
                    &mut basis,
                    &mut case.store,
                    &mut case.history,
                    InferenceInput {
                        scope: &case.scope,
                        pack: &case.pack,
                        event_id: event,
                        occurred_at_ms: 1,
                        source_mode: "blind".into(),
                    },
                )
            });
            assert!(
                relay.observed(Duration::from_secs(5)),
                "COMMIT request observed"
            );
            assert!(
                relay.consumed(Duration::from_secs(5)),
                "server confirmed durable COMMIT before ACK loss"
            );
            assert_eq!(count("execution.inference_reservations", &s), 1);
            let expected_calls = usize::from(phase == "publication" || phase == "history");
            assert_eq!(calls.load(Ordering::SeqCst), expected_calls);
            assert_eq!(
                count("execution.inference_calls", &s),
                i64::from(phase != "reservation")
            );
            assert_eq!(
                count("cognition.reasoning_decisions", &s),
                i64::from(phase == "publication" || phase == "history")
            );
            assert_eq!(
                count("trajectory.reasoning_history", &s),
                i64::from(phase == "history")
            );
            if phase == "publication" {
                assert_eq!(native::admin().query_one("SELECT count(*) FROM cognition.reasoning_outbox o JOIN cognition.reasoning_decisions q USING(event) WHERE q.d=$1",&[&s.decision]).unwrap().get::<_,i64>(0),1);
            }
            drop(relay); // physically close the socket after durable observation
            worker.join().unwrap()
        });
        assert!(!receipt.dispatch_granted);
        let recovered = recover(&q, &s);
        assert_eq!(count("cognition.reasoning_decisions", &s), 1);
        assert_eq!(count("trajectory.reasoning_history", &s), 1);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            usize::from(phase == "publication" || phase == "history")
        );
        let charged: Value = native::client("DW_INFERENCE_BROKER_DATABASE_URL")
            .query_one(
                "SELECT execution.inference_capacity_snapshot($1,$2)",
                &[&s.engagement, &s.campaign],
            )
            .unwrap()
            .get(0);
        assert_eq!(
            charged["charged"]["calls"], 2,
            "lost ACK never refunds reserved capacity"
        );
        if phase == "may_send" {
            assert_eq!(
                receipt.consumption, "unknown",
                "failed connection cannot establish current accounting"
            );
            assert_eq!(recovered["consumption"], "uncertain_charged");
        }
    }
}

#[test]
fn rolled_back_marker_has_no_durable_call() {
    let _guard = native::db();
    let mut case = native::decision(0x9905);
    let r = case.broker.reserve(&case.scope, &case.pack).unwrap();
    let b = request::bind(&case.pack, None).unwrap();
    let mut connection = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let mut tx = connection.transaction().unwrap();
    tx.query_one(
        "SELECT execution.inference_may_send($1,$2,$3,$4,$5,$6,$7,$8)",
        &[
            &case.scope.engagement,
            &case.scope.campaign,
            &case.scope.decision,
            &r.generation_call,
            &std::str::from_utf8(&b.pack).unwrap(),
            &std::str::from_utf8(&b.configuration).unwrap(),
            &std::str::from_utf8(&b.body).unwrap(),
            &b.digest,
        ],
    )
    .unwrap();
    assert_eq!(
        count("execution.inference_calls", &case.scope),
        0,
        "uncommitted marker is invisible from fresh connection"
    );
    tx.rollback().unwrap();
    assert_eq!(count("execution.inference_calls", &case.scope), 0);
    recover(&case.request, &case.scope);
    assert_eq!(count("execution.inference_calls", &case.scope), 0);
    assert_eq!(count("cognition.reasoning_decisions", &case.scope), 0);
}

#[test]
fn process_exit_after_release_preserves_durable_charge_and_never_replays() {
    use std::io::BufRead;
    let _guard = native::db();
    if let Some(path) = std::env::var_os("DW_NATIVE_PROCESS_EXIT") {
        let mut case = native::decision(0x9907);
        std::fs::write(
            path,
            serde_json::to_vec(&json!({"request":case.request,"decision":case.scope.decision}))
                .unwrap(),
        )
        .unwrap();
        let mut basis = Basis {
            client: native::client("DW_INFERENCE_BROKER_DATABASE_URL"),
            scope: case.scope.clone(),
            references: request::reference_ids(&case.pack).unwrap(),
            deadline: case.broker.deadline,
        };
        let event =
            duskweave::postgres_mission::PgAllocator::new(native::client("DW_TEST_DATABASE_URL"))
                .allocate()
                .unwrap();
        duskweave::m1_reasoning::inference::infer(
            &mut case.broker,
            &mut ProviderFixture(Arc::new(AtomicUsize::new(0))),
            &mut basis,
            &mut case.store,
            &mut case.history,
            InferenceInput {
                scope: &case.scope,
                pack: &case.pack,
                event_id: event,
                occurred_at_ms: 1,
                source_mode: "blind".into(),
            },
        );
        panic!("parent must terminate the released native process");
    }
    let path = std::env::temp_dir().join(format!("dw-native-exit-{}.json", std::process::id()));
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .env("DW_NATIVE_PROCESS_EXIT", &path)
        .args([
            "--exact",
            "process_exit_after_release_preserves_durable_charge_and_never_replays",
            "--nocapture",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let output = child.stdout.take().unwrap();
    let (ready, wait) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in std::io::BufReader::new(output)
            .lines()
            .map_while(Result::ok)
        {
            if line == "native_exit_released" {
                ready.send(()).unwrap();
                break;
            }
        }
    });
    let released = wait.recv_timeout(Duration::from_secs(10)).is_ok();
    child.kill().unwrap();
    child.wait().unwrap();
    reader.join().unwrap();
    assert!(
        released,
        "child must reach confirmed may-send and provider release before termination"
    );
    let state: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let q: duskweave::m1_context_input::ContextRequest =
        serde_json::from_value(state["request"].clone()).unwrap();
    let s = Scope {
        engagement: q.mission.engagement_id.0,
        campaign: q.mission.campaign_id.0,
        decision: serde_json::from_value(state["decision"].clone()).unwrap(),
        mission_revision: 1,
        permission: q.mission.permission_id,
    };
    assert_eq!(count("execution.inference_reservations", &s), 1);
    assert_eq!(count("execution.inference_calls", &s), 1);
    assert_eq!(count("cognition.reasoning_decisions", &s), 0);
    assert_eq!(count("trajectory.reasoning_history", &s), 0);
    let receipt = recover(&state["request"], &s);
    assert_eq!(receipt["consumption"], "uncertain_charged");
    assert_eq!(receipt["publication"], "absent_not_proof_of_no_send");
    assert_eq!(receipt["history"], "unavailable");
    assert_eq!(count("execution.inference_calls", &s), 1);
    assert_eq!(count("cognition.reasoning_decisions", &s), 0);
    assert_eq!(count("trajectory.reasoning_history", &s), 0);
    let capacity: Value = native::client("DW_INFERENCE_BROKER_DATABASE_URL")
        .query_one(
            "SELECT execution.inference_capacity_snapshot($1,$2)",
            &[&s.engagement, &s.campaign],
        )
        .unwrap()
        .get(0);
    assert_eq!(capacity["charged"]["calls"], 2);
    std::fs::remove_file(path).unwrap();
}
