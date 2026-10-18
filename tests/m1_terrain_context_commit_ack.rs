//! Physical PostgreSQL COMMIT ACK loss on Terrain-informed inference decisions and history.
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/database_wait_proxy.rs"]
mod proxy;
#[path = "support/m1_terrain_context.rs"]
mod tc_support;

use duskweave::m1_context;
use duskweave::m1_context_input::Priority;
use duskweave::m1_reasoning::{
    inference::{self, InferenceInput, Provider, Received, Scope},
    postgres::{Basis, PgBroker, PgDecisionStore},
    postgres_terrain::TerrainBasis,
    proposal::ProposalBasis,
    request::{self, BoundCall},
    response,
};
use duskweave::postgres_inference_budget::PgInferenceReservationStore;
use duskweave::postgres_mission::{PgContextReader, PgMissionStore};
use duskweave::postgres_trajectory::PgTrajectory;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use tc_support::terrain_support;
use uuid::Uuid;

struct TerrainCommitAckProvider(Arc<AtomicUsize>);
impl Provider for TerrainCommitAckProvider {
    fn prepare(&mut self, _: &BoundCall, _: Instant) -> Result<(), &'static str> {
        Ok(())
    }

    fn call(
        &mut self,
        b: &BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        release()?;
        self.0.fetch_add(1, Ordering::SeqCst);
        let refs = request::reference_ids(&b.pack)?;
        assert!(
            refs.iter().any(|r| r == "terrain_0"),
            "pack must include terrain_0"
        );
        let proposal = json!({
            "version": "m1_inference_proposal_v1",
            "hypothesis": "Terrain-qualified DNS summary supports candidate access validation",
            "considered_alternative": "Defer until additional vantage observation",
            "references": ["mission", "asset_0", "terrain_0"],
            "expected_observation": "Consistent HTTPS reachability under permitted envelope",
            "falsifying_observation": "Applicability conflict or authority withdrawal",
            "uncertainty": "Bounded single-vantage TTL window",
            "rationale": "Owner-qualified DNS summary is fresh and unconflicted",
            "next_intent": {
                "category": "observe_more",
                "references": ["asset_0", "terrain_0"],
                "manifest_reference": null,
                "view_request": null
            }
        });
        let envelope = json!({
            "status": "completed",
            "model": "deepseek-flash",
            "output": [{
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "content": [{
                    "type": "output_text",
                    "text": proposal.to_string()
                }]
            }],
            "usage": {
                "input_tokens": 18,
                "output_tokens": 28
            }
        });
        response::admit_envelope(
            &serde_json::to_vec(&envelope).unwrap(),
            100,
            Duration::ZERO,
            basis,
        )
    }
}

fn count_rows(table: &str, s: &Scope) -> i64 {
    let (e_col, c_col, d_col) =
        if table == "trajectory.reasoning_history" || table == "execution.inference_reservations" {
            ("engagement_id", "campaign_id", "operation_id")
        } else {
            ("e", "c", "d")
        };
    terrain_support::admin_client()
        .query_one(
            &format!("SELECT count(*) FROM {table} WHERE {e_col}=$1 AND {c_col}=$2 AND {d_col}=$3"),
            &[&s.engagement, &s.campaign, &s.decision],
        )
        .unwrap()
        .get(0)
}

fn cli_inspect_and_recover(request: &Value, s: &Scope) -> (Value, Value, Value) {
    let path = std::env::temp_dir().join(format!("dw-tc-ack-{}.json", s.decision));
    std::fs::write(&path, serde_json::to_vec(request).unwrap()).unwrap();
    let mut results = Vec::new();
    for command in ["m1-infer-inspect", "m1-infer-recover", "m1-infer-recover"] {
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_duskweave"));
        child
            .env_clear()
            .env("DW_DATABASE_CONFIG_MODE", "env-local")
            .env(
                "DW_DATABASE_URL",
                std::env::var("DW_TEST_DATABASE_URL").unwrap(),
            );
        for var in [
            "DW_INFERENCE_BROKER_DATABASE_URL",
            "DW_COGNITION_DATABASE_URL",
            "DW_TRAJECTORY_DATABASE_URL",
            "DW_TERRAIN_DATABASE_URL",
            "SystemRoot",
            "LLVM_PROFILE_FILE",
        ] {
            if let Some(val) = std::env::var_os(var) {
                child.env(var, val);
            }
        }
        let out = child
            .arg(command)
            .arg("--input")
            .arg(&path)
            .arg("--operation")
            .arg(s.decision.to_string())
            .output()
            .unwrap();
        assert!(
            out.status.success() && out.stderr.is_empty(),
            "recovery command {command} must succeed"
        );
        let rec: Value = serde_json::from_slice(
            out.stdout
                .split(|b| *b == b'\n')
                .find(|l| l.first() == Some(&b'{'))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(rec["decision"], s.decision.to_string());
        assert_eq!(rec["recovery_category"], "no_provider_replay");
        assert_eq!(rec["dispatch_granted"], false);
        results.push(rec);
    }
    let _ = std::fs::remove_file(path);
    (results.remove(0), results.remove(0), results.remove(0))
}

#[test]
fn terrain_informed_decision_and_history_survive_physical_commit_ack_loss() {
    let _g = terrain_support::db();
    for (idx, phase) in ["reservation", "may_send", "publication", "history"]
        .into_iter()
        .enumerate()
    {
        let (e, c, _, raw_reg, now) =
            tc_support::register_combined_campaign(0xc100 + idx as u128, true);
        let a1 = Uuid::from_u128(0x21);
        let dns = terrain_support::dns_request_json(e, c, now - 10);
        let _ = tc_support::record_dns_accepted(e, c, &dns);

        let (req_val, q) =
            tc_support::context_request_for(e, c, &raw_reg, vec![(a1, Priority::High)]);
        let dec_op = terrain_support::allocate_op();
        let scope = Scope {
            engagement: e.0,
            campaign: c.0,
            decision: dec_op.0,
            mission_revision: 1,
            permission: q.mission.permission_id,
        };
        let input_hash = terrain_support::native::bind_request(&req_val, &scope);
        let prepared = m1_context::prepare(
            &mut PgContextReader::new(terrain_support::broker_client()),
            &q,
            dec_op,
        )
        .expect("prepare terrain pack");
        let pack_utf8 = prepared.utf8().to_owned();
        let deps = prepared.terrain_dependencies().to_vec();
        assert_eq!(deps.len(), 1);

        let relay = proxy::Relay::start("127.0.0.1:5432".parse().unwrap(), b"COMMIT");
        let mut broker = PgBroker {
            client: if phase == "may_send" {
                terrain_support::through("DW_INFERENCE_BROKER_DATABASE_URL", relay.port())
            } else {
                terrain_support::broker_client()
            },
            mission: PgMissionStore::new(terrain_support::runtime_client()),
            reservations: PgInferenceReservationStore::new(if phase == "reservation" {
                terrain_support::through("DW_INFERENCE_BROKER_DATABASE_URL", relay.port())
            } else {
                terrain_support::broker_client()
            }),
            request: q.mission.clone(),
            deadline: Instant::now() + Duration::from_secs(20),
        };
        let mut store = PgDecisionStore {
            client: if phase == "publication" {
                terrain_support::through("DW_COGNITION_DATABASE_URL", relay.port())
            } else {
                terrain_support::cognition_client()
            },
            input_hash,
        };
        let mut history = PgTrajectory::new(if phase == "history" {
            terrain_support::through("DW_TRAJECTORY_DATABASE_URL", relay.port())
        } else {
            terrain_support::trajectory_client()
        });

        let calls = Arc::new(AtomicUsize::new(0));
        let calls_worker = calls.clone();
        let scope_worker = scope.clone();
        let pack_worker = pack_utf8.clone().into_bytes();
        let event_id = terrain_support::allocate_op().0;

        let receipt = std::thread::scope(|threads| {
            let worker = threads.spawn(move || {
                let mut basis = TerrainBasis {
                    mission: Basis {
                        client: terrain_support::broker_client(),
                        scope: scope_worker.clone(),
                        references: request::reference_ids(&pack_worker).unwrap(),
                        deadline: broker.deadline,
                    },
                    dependencies: deps,
                };
                inference::infer(
                    &mut broker,
                    &mut TerrainCommitAckProvider(calls_worker),
                    &mut basis,
                    &mut store,
                    &mut history,
                    InferenceInput {
                        scope: &scope_worker,
                        pack: &pack_worker,
                        event_id,
                        occurred_at_ms: (now as u64) * 1000,
                        source_mode: "blind".into(),
                    },
                )
            });
            assert!(relay.observed(Duration::from_secs(5)), "COMMIT observed");
            assert!(
                relay.consumed(Duration::from_secs(5)),
                "durable COMMIT before ACK loss"
            );
            assert_eq!(count_rows("execution.inference_reservations", &scope), 1);
            assert_eq!(
                count_rows("execution.inference_calls", &scope),
                i64::from(phase != "reservation")
            );
            assert_eq!(
                count_rows("cognition.reasoning_decisions", &scope),
                i64::from(phase == "publication" || phase == "history")
            );
            assert_eq!(
                count_rows("trajectory.reasoning_history", &scope),
                i64::from(phase == "history")
            );
            if phase == "publication" {
                let outbox_count: i64 = terrain_support::admin_client()
                    .query_one(
                        "SELECT count(*) FROM cognition.reasoning_outbox WHERE event = $1",
                        &[&event_id],
                    )
                    .unwrap()
                    .get(0);
                assert_eq!(outbox_count, 1);
            }
            drop(relay);
            worker.join().unwrap()
        });

        assert!(!receipt.dispatch_granted);
        if phase == "may_send" {
            // Broker connection was severed by ACK loss during infer, so infer receipt reports unavailable
            assert_eq!(receipt.pack_state, "unavailable");
            assert!(receipt.pack_utf8.is_none());
        }

        let (inspected, recovered1, recovered2) = cli_inspect_and_recover(&req_val, &scope);
        assert_eq!(count_rows("cognition.reasoning_decisions", &scope), 1);
        assert_eq!(count_rows("trajectory.reasoning_history", &scope), 1);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            usize::from(phase == "publication" || phase == "history")
        );

        for rec in [&inspected, &recovered1, &recovered2] {
            assert_eq!(rec["event"], event_id.to_string());
            if phase == "reservation" {
                assert_eq!(rec["pack_state"], "not_bound");
                assert!(rec["pack_utf8"].is_null());
                assert!(rec["proposal"].is_null());
            } else {
                assert_eq!(rec["pack_state"], "available");
                assert_eq!(rec["pack_utf8"], pack_utf8);
            }
            if phase == "publication" || phase == "history" {
                assert_eq!(rec["proposal_disposition"], "validated_proposal");
                assert_eq!(
                    rec["proposal"]["references"],
                    json!(["mission", "asset_0", "terrain_0"])
                );
            }
        }
        assert_eq!(recovered1["history"], "completed");
        assert_eq!(recovered2["history"], "completed");
    }
}
