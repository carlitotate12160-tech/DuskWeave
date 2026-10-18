//! Integration tests for CLI preparation/inspection/recovery and historical pack/proposal contracts.
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_terrain_context.rs"]
mod tc_support;

use duskweave::m1_context::{self, ContextBasis, TerrainContextState};
use duskweave::m1_context_capacity::ContextCapacity;
use duskweave::m1_context_input::Priority;
use duskweave::m1_inference_budget::ReservedCapacity;
use duskweave::m1_reasoning::{
    inference::{self, InferenceInput, Provider, Received, Scope, inspect_recover},
    postgres::{Basis, PgBroker, PgDecisionStore},
    postgres_terrain::TerrainBasis,
    proposal::ProposalBasis,
    request::{self, BoundCall},
    response,
};
use duskweave::mission::{AssetRef, OperationId};
use duskweave::postgres_inference_budget::PgInferenceReservationStore;
use duskweave::postgres_mission::{PgContextReader, PgMissionStore};
use duskweave::postgres_terrain_view;
use duskweave::postgres_trajectory::PgTrajectory;
use serde_json::{Value, json};
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use tc_support::terrain_support;
use uuid::Uuid;

fn run_cli_raw(command: &str, args: &[&str]) -> (bool, Vec<Value>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    child
        .env_clear()
        .env("DW_DATABASE_CONFIG_MODE", "env-local")
        .env(
            "DW_DATABASE_URL",
            std::env::var("DW_NATIVE_PREPARE_DATABASE_URL").expect("prepare DSN"),
        );
    if command != "m1-context-prepare" {
        child.env(
            "DW_DATABASE_URL",
            std::env::var("DW_TEST_DATABASE_URL").expect("runtime DSN"),
        );
    }
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
        .args(args)
        .output()
        .expect("execute duskweave CLI");
    assert!(out.stderr.is_empty(), "stderr must be empty: {out:?}");
    let ok = out.status.success();
    let lines: Vec<Value> = out
        .stdout
        .split(|b| *b == b'\n')
        .filter(|l| !l.iter().all(u8::is_ascii_whitespace))
        .filter(|l| ok || l.first() == Some(&b'{'))
        .map(|l| serde_json::from_slice(l).expect("JSON receipt line"))
        .collect();
    (ok, lines)
}

fn write_temp_input(val: &Value, tag: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "dw-tc-cli-{tag}-{}-{}.json",
        std::process::id(),
        val["mission"]["campaign_id"].as_str().unwrap()
    ));
    std::fs::write(&path, serde_json::to_vec(val).unwrap()).unwrap();
    path
}

struct TerrainAwareProvider {
    calls: Arc<AtomicUsize>,
    seen_packs: Vec<String>,
    repair_first: bool,
    cite_refs: Vec<String>,
}

impl Provider for TerrainAwareProvider {
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
        let call_no = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let pack_str = std::str::from_utf8(&b.pack)
            .map_err(|_| "pack_utf8")?
            .to_owned();
        self.seen_packs.push(pack_str.clone());
        if self.repair_first && call_no == 1 {
            let bad = b"HTTP/1.1 200 OK\r\nContent-Encoding: identity\r\n\r\n{\"status\":\"completed\",\"model\":\"deepseek-flash\",\"output\":[{\"type\":\"message\",\"role\":\"assistant\",\"status\":\"completed\",\"content\":[{\"type\":\"output_text\",\"text\":\"{\\\"version\\\":\\\"wrong\\\"}\"}]}],\"usage\":{\"input_tokens\":8,\"output_tokens\":8}}";
            let mut diag =
                duskweave::m1_reasoning::provider_diagnostic::ProviderDiagnostic::default();
            return response::admit_http(bad, Duration::ZERO, basis, &mut diag);
        }
        let proposal = json!({
            "version": "m1_inference_proposal_v1",
            "hypothesis": "Terrain-qualified DNS summary supports bounded access-validation candidate selection",
            "considered_alternative": "Defer candidate selection until additional vantage observation",
            "references": self.cite_refs,
            "expected_observation": "Consistent HTTPS endpoint reachability within permitted envelope",
            "falsifying_observation": "DNS applicability conflict or authority withdrawal before contact",
            "uncertainty": "Single-vantage DNS summary with bounded TTL window",
            "rationale": "Owner-qualified DNS claim is fresh, uncorroborated Tier1, and unconflicted",
            "next_intent": {
                "category": "observe_more",
                "references": self.cite_refs,
                "manifest_reference": null,
                "view_request": {
                    "owner": "cyber_terrain",
                    "selectors": ["asset_0"]
                }
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
                "input_tokens": 24,
                "output_tokens": 36
            }
        });
        response::admit_envelope(
            &serde_json::to_vec(&envelope).unwrap(),
            120,
            Duration::ZERO,
            basis,
        )
    }
}

#[test]
fn cli_context_prepare_with_terrain_view_matches_exact_reconstructed_bytes() {
    let _g = terrain_support::db();
    let (e, c, mission, raw_reg, now) = tc_support::register_combined_campaign(0xb101, true);
    let a1 = Uuid::from_u128(0x21);
    let dns = terrain_support::dns_request_json(e, c, now - 10);
    let _ = tc_support::record_dns_accepted(e, c, &dns);

    let (req_val, q) = tc_support::context_request_for(e, c, &raw_reg, vec![(a1, Priority::High)]);
    let path = write_temp_input(&req_val, "prepare");
    let (ok, lines) = run_cli_raw("m1-context-prepare", &["--input", path.to_str().unwrap()]);
    assert!(ok, "m1-context-prepare must succeed: {lines:?}");
    let r = lines.last().unwrap();
    assert_eq!(r["outcome"], "prepared");
    assert_eq!(r["dispatch_granted"], false);

    let decision_id = Uuid::parse_str(r["decision_id"].as_str().unwrap()).unwrap();
    let pack_str = r["pack_utf8"].as_str().unwrap();
    for forbidden in ["www.example.invalid", "192.0.2.53", "198.51.100.10"] {
        assert!(!pack_str.contains(forbidden));
    }

    let parsed: Value = serde_json::from_str(pack_str).unwrap();
    assert_eq!(parsed["owner_views"][1]["owner"], "cyber_terrain");
    assert_eq!(parsed["owner_views"][1]["availability"], "available");
    assert_eq!(parsed["owner_views"][1]["shown_claims"], 1);
    assert_eq!(
        parsed["candidates"][0]["terrain_references"],
        json!(["terrain_0"])
    );

    // Reconstruct exact PreparedContext via core API and verify byte-for-byte equality
    let eval_at = parsed["evaluated_at"].as_i64().unwrap();
    let epoch = parsed["remaining_capacity"]["snapshot_epoch"]
        .as_i64()
        .unwrap();
    let mut broker = terrain_support::broker_client();
    let view = postgres_terrain_view::read(&mut broker, &mission, &[AssetRef(a1)], None)
        .unwrap()
        .evaluate_at(eval_at, true);
    struct ReconstructedReader(ContextBasis);
    impl m1_context::ContextReader for ReconstructedReader {
        fn read(
            &mut self,
            _: &duskweave::m1_context_input::ContextRequest,
            _: OperationId,
        ) -> duskweave::Res<ContextBasis> {
            Ok(ContextBasis {
                mission: self.0.mission.clone(),
                capacity: ContextCapacity {
                    version: self.0.capacity.version.clone(),
                    epoch: self.0.capacity.epoch,
                    session_claimed: false,
                    active_holds: 0,
                    charged: self.0.capacity.charged.clone(),
                },
                evaluated_at: self.0.evaluated_at,
                terrain: self.0.terrain.clone(),
            })
        }
    }
    let mut reader = ReconstructedReader(ContextBasis {
        mission,
        capacity: ContextCapacity {
            version: "m1_inference_capacity_snapshot_v1".into(),
            epoch,
            session_claimed: false,
            active_holds: 0,
            charged: ReservedCapacity {
                calls: 0,
                request_bytes: 0,
                response_bytes: 0,
                output_token_capacity: 0,
                wall_time_ms: 0,
            },
        },
        evaluated_at: eval_at,
        terrain: TerrainContextState::Available(view),
    });
    let expected = m1_context::prepare(&mut reader, &q, OperationId(decision_id)).unwrap();
    assert_eq!(pack_str.as_bytes(), expected.utf8().as_bytes());

    // A second preparation command allocates a distinct decision_id
    let (ok2, lines2) = run_cli_raw("m1-context-prepare", &["--input", path.to_str().unwrap()]);
    assert!(ok2);
    assert_ne!(lines2.last().unwrap()["decision_id"], r["decision_id"]);
    let _ = std::fs::remove_file(path);
}

#[test]
fn terrain_inference_repair_and_cli_inspect_recover_preserve_exact_pack_and_detect_tampering() {
    let _g = terrain_support::db();
    let (e, c, mission, raw_reg, now) = tc_support::register_combined_campaign(0xb102, true);
    let a1 = Uuid::from_u128(0x21);
    let dns1 = terrain_support::dns_request_json(e, c, now - 10);
    let (op1, _, _, _) = tc_support::record_dns_accepted(e, c, &dns1);

    let (req_val, q) = tc_support::context_request_for(e, c, &raw_reg, vec![(a1, Priority::High)]);
    let path = write_temp_input(&req_val, "infer_repair");

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
    .expect("prepare context");
    assert_eq!(prepared.terrain_dependencies().len(), 1);
    let assembled_pack = prepared.utf8().to_owned();

    let mut broker = PgBroker {
        client: terrain_support::broker_client(),
        mission: PgMissionStore::new(terrain_support::runtime_client()),
        reservations: PgInferenceReservationStore::new(terrain_support::broker_client()),
        request: q.mission.clone(),
        deadline: Instant::now() + Duration::from_secs(20),
    };
    let mut store = PgDecisionStore {
        client: terrain_support::cognition_client(),
        input_hash,
    };
    let mut history = PgTrajectory::new(terrain_support::trajectory_client());
    let mut basis = TerrainBasis {
        mission: Basis {
            client: terrain_support::broker_client(),
            scope: scope.clone(),
            references: request::reference_ids(assembled_pack.as_bytes()).unwrap(),
            deadline: broker.deadline,
        },
        dependencies: prepared.terrain_dependencies().to_vec(),
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let mut provider = TerrainAwareProvider {
        calls: calls.clone(),
        seen_packs: Vec::new(),
        repair_first: true,
        cite_refs: vec!["mission".into(), "asset_0".into(), "terrain_0".into()],
    };
    let event_id = terrain_support::allocate_op().0;
    let receipt = inference::infer(
        &mut broker,
        &mut provider,
        &mut basis,
        &mut store,
        &mut history,
        InferenceInput {
            scope: &scope,
            pack: assembled_pack.as_bytes(),
            event_id,
            occurred_at_ms: (now as u64) * 1000,
            source_mode: "blind".into(),
        },
    );

    // Generation + 1 repair both received identical P bytes
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(provider.seen_packs.len(), 2);
    assert_eq!(provider.seen_packs[0], assembled_pack);
    assert_eq!(provider.seen_packs[1], assembled_pack);
    assert_eq!(receipt.proposal_disposition, "validated_proposal");
    assert_eq!(receipt.pack_state, "available");
    assert_eq!(receipt.pack_utf8.as_deref(), Some(assembled_pack.as_str()));
    assert!(receipt.proposal.is_some());
    assert_eq!(
        receipt.proposal.as_ref().unwrap().references,
        vec!["mission", "asset_0", "terrain_0"]
    );
    assert!(!receipt.dispatch_granted);

    // Now invalidate the Terrain claim (via integrity hold) AND withdraw the Mission:
    // historical CLI m1-infer-inspect and m1-infer-recover must still succeed without provider calls
    // and return the exact original pack_utf8 and proposal!
    let mut conflict_dns = dns1;
    conflict_dns["perceived_at"] = json!(now - 1);
    conflict_dns["answers"] = json!([{"ipv4": "198.51.100.88", "ttl_seconds": 60}]);
    tc_support::inject_integrity_hold(e, c, op1, &conflict_dns);
    let marker = terrain_support::policy_support::marker(&mission);
    terrain_support::policy_support::insert_marker(&mut terrain_support::runtime_client(), &marker);

    for cmd in ["m1-infer-inspect", "m1-infer-recover", "m1-infer-recover"] {
        let (ok, out_lines) = run_cli_raw(
            cmd,
            &[
                "--input",
                path.to_str().unwrap(),
                "--operation",
                &scope.decision.to_string(),
            ],
        );
        assert!(ok, "{cmd} must succeed after withdrawal/conflict");
        let rec = out_lines.last().unwrap();
        assert_eq!(rec["decision"], scope.decision.to_string());
        assert_eq!(rec["event"], event_id.to_string());
        assert_eq!(rec["pack_state"], "available");
        assert_eq!(rec["pack_utf8"], assembled_pack);
        assert_eq!(
            rec["proposal"]["references"],
            json!(["mission", "asset_0", "terrain_0"])
        );
        assert_eq!(rec["history"], "completed");
        assert_eq!(rec["dispatch_granted"], false);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    // Pack inspection alone is unavailable; accounting/history stay readable.
    // Collect every result and restore grants before asserting their outcomes.
    let mut admin = terrain_support::admin_client();
    let res_before: i64 = admin
        .query_one(
            "SELECT count(*) FROM execution.inference_reservations WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap()
        .get(0);
    admin.batch_execute("REVOKE SELECT ON execution.inference_calls FROM dw_m1_broker; GRANT SELECT(e,c,d,state) ON execution.inference_calls TO dw_m1_broker").unwrap();
    let direct_unavail = inspect_recover(&scope, false, &mut broker, &mut store, &mut history);
    let unavailable: Vec<_> = ["m1-infer-inspect", "m1-infer-recover", "m1-infer"]
        .into_iter()
        .map(|cmd| {
            run_cli_raw(
                cmd,
                &[
                    "--input",
                    path.to_str().unwrap(),
                    "--operation",
                    &scope.decision.to_string(),
                ],
            )
        })
        .collect();
    admin.batch_execute("REVOKE SELECT(e,c,d,state) ON execution.inference_calls FROM dw_m1_broker; GRANT SELECT ON execution.inference_calls TO dw_m1_broker").unwrap();
    let res_after: i64 = admin
        .query_one(
            "SELECT count(*) FROM execution.inference_reservations WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap()
        .get(0);
    assert_eq!(res_before, res_after);
    let direct_rec =
        direct_unavail.expect("direct inspect_recover must return unavailable receipt");
    assert_eq!(direct_rec.pack_state, "unavailable");
    assert!(direct_rec.pack_utf8.is_none());
    assert_eq!(
        direct_rec.consumption,
        "completed_charged_unused_repair_reserved"
    );
    assert!(!direct_rec.dispatch_granted);
    for (ok, lines) in unavailable {
        assert!(
            ok,
            "pack-only read failure must yield unavailable receipt: {lines:?}"
        );
        let rec = lines.last().unwrap();
        assert_eq!(rec["decision"], scope.decision.to_string());
        assert_eq!(rec["pack_state"], "unavailable");
        assert!(rec["pack_utf8"].is_null());
        assert_eq!(rec["event"], event_id.to_string());
        assert_eq!(rec["proposal_disposition"], "validated_proposal");
        assert_eq!(rec["publication"], "committed");
        assert_eq!(rec["history"], "completed");
        assert_eq!(
            rec["consumption"],
            "completed_charged_unused_repair_reserved"
        );
        assert_eq!(
            rec["proposal"]["references"],
            json!(["mission", "asset_0", "terrain_0"])
        );
        assert_eq!(rec["recovery_category"], "no_provider_replay");
        assert_eq!(rec["dispatch_granted"], false);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    // Mutating stored pack bytes in execution.inference_calls is detected as inference_decision_integrity
    terrain_support::admin_client()
        .execute(
            "UPDATE execution.inference_calls SET p = '{\"references\":[]}' WHERE d = $1",
            &[&scope.decision],
        )
        .unwrap();
    assert_eq!(
        inspect_recover(&scope, false, &mut broker, &mut store, &mut history).err(),
        Some("inference_decision_integrity")
    );

    let _ = std::fs::remove_file(path);
}
