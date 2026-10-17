//! PostgreSQL integration tests for restricted Terrain owner-read, lineage checks, and current-use recheck.
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_terrain_context.rs"]
mod tc_support;

use duskweave::Fail;
use duskweave::m1_context;
use duskweave::m1_context_input::Priority;
use duskweave::m1_reasoning::{
    inference::{self, Broker, InferenceInput, Provider, Received, Scope},
    postgres::{Basis, PgBroker, PgDecisionStore, qualify_owner},
    postgres_terrain::TerrainBasis,
    proposal::ProposalBasis,
    request::{self, BoundCall},
    response,
};
use duskweave::m1_terrain_view::verify_current_dependencies;
use duskweave::mission::{AssetRef, OperationId};
use duskweave::postgres_mission::PgContextReader;
use duskweave::postgres_terrain_view;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tc_support::terrain_support;
use uuid::Uuid;

#[test]
fn restricted_broker_reads_safe_view_while_private_tables_mutations_and_hostile_roles_fail() {
    let _g = terrain_support::db();
    let (e, c, mission, _, now) = tc_support::register_combined_campaign(0xa101, true);
    let a1 = AssetRef(Uuid::from_u128(0x21));

    let dns_val = terrain_support::dns_request_json(e, c, now - 10);
    let (_, accepted_ev, _, _) = tc_support::record_dns_accepted(e, c, &dns_val);

    // 1. Qualified broker reads public view without private table privileges
    let mut broker = terrain_support::broker_client();
    assert!(qualify_owner(&mut broker, "dw_m1_broker").is_ok());
    let view = postgres_terrain_view::read(&mut broker, &mission, &[a1], None)
        .expect("broker reads terrain public view");
    assert_eq!(view.claims.len(), 1);
    assert_eq!(view.claims[0].claim_id, accepted_ev.claim.claim_id);
    assert!(view.claims[0].routing_usable);

    let view_json = serde_json::to_string(&view).unwrap();
    for forbidden in ["www.example.invalid", "192.0.2.53", "198.51.100.10"] {
        assert!(!view_json.contains(forbidden));
    }

    // 2. Broker cannot SELECT or mutate any private terrain table
    for table in [
        "terrain.observations",
        "terrain.materials",
        "terrain.evaluations",
        "terrain.claim_revisions",
        "terrain.current_claims",
        "terrain.outbox",
        "terrain.claim_integrity_holds",
        "terrain.integrity_conflicts",
    ] {
        assert!(
            broker
                .query(&format!("SELECT * FROM {table}"), &[])
                .is_err(),
            "broker must not read private table {table}"
        );
        assert!(
            broker
                .execute(&format!("DELETE FROM {table}"), &[])
                .is_err(),
            "broker must not mutate {table}"
        );
    }

    // 3. Other runtime/owner roles cannot execute terrain.current_dns_context_view
    let query_json = json!({
        "version": "m1_terrain_context_view_query_v1",
        "engagement_id": e,
        "campaign_id": c,
        "operator_ref": Uuid::from_u128(0x11),
        "expected_mission_revision": 1,
        "goal_ref": Uuid::from_u128(0x13),
        "exercise_mode": "blind",
        "selected_assets": [a1.0],
        "claim_ids": null
    });
    for (var, expected_role) in [
        ("DW_TEST_DATABASE_URL", "dw_runtime"),
        ("DW_COGNITION_DATABASE_URL", "dw_m1_cognition"),
        ("DW_TRAJECTORY_DATABASE_URL", "dw_m1_trajectory"),
        ("DW_TERRAIN_DATABASE_URL", "dw_m1_terrain"),
    ] {
        let mut client = terrain_support::native::client(var);
        assert!(qualify_owner(&mut client, expected_role).is_ok());
        assert!(
            client
                .query_one(
                    "SELECT terrain.current_dns_context_view($1)",
                    &[&query_json]
                )
                .is_err(),
            "{var} must be denied EXECUTE on terrain.current_dns_context_view"
        );
    }

    // 4. Hostile principals (sibling, column, setonly, foreign, corebad, terraincolumn, terrainsetonly) fail
    for (var, role) in [
        ("DW_NATIVE_SIBLING_DATABASE_URL", "dw_m1_broker"),
        ("DW_NATIVE_COLUMN_DATABASE_URL", "dw_m1_broker"),
        ("DW_NATIVE_SETONLY_DATABASE_URL", "dw_m1_cognition"),
        ("DW_NATIVE_FOREIGN_DATABASE_URL", "dw_m1_cognition"),
        ("DW_NATIVE_COREBAD_DATABASE_URL", "dw_runtime"),
        ("DW_NATIVE_TERRAINCOLUMN_DATABASE_URL", "dw_m1_terrain"),
        ("DW_NATIVE_TERRAINCOLUMN_DATABASE_URL", "dw_m1_broker"),
        ("DW_NATIVE_TERRAINSETONLY_DATABASE_URL", "dw_m1_terrain"),
        ("DW_NATIVE_TERRAINSETONLY_DATABASE_URL", "dw_m1_broker"),
    ] {
        let mut bad = terrain_support::native::client(var);
        assert!(
            qualify_owner(&mut bad, role).is_err(),
            "hostile {var} must fail qualify_owner({role})"
        );
        assert!(
            bad.query_one(
                "SELECT terrain.current_dns_context_view($1)",
                &[&query_json]
            )
            .is_err(),
            "hostile {var} must be rejected by terrain.current_dns_context_view"
        );
    }
}

#[test]
fn evidence_revision_withholding_hold_and_withdrawal_update_context_and_dependencies() {
    let _g = terrain_support::db();
    let (e, c, mission, raw_reg, now) = tc_support::register_combined_campaign(0xa102, true);
    let a1 = Uuid::from_u128(0x21);
    let a2 = Uuid::from_u128(0x22);
    let (_, q) = tc_support::context_request_for(
        e,
        c,
        &raw_reg,
        vec![(a1, Priority::High), (a2, Priority::Low)],
    );

    // 1. Fresh DNS observation on asset 0x21 -> pack A includes terrain_0 (revision 1)
    let dns1 = terrain_support::dns_request_json(e, c, now - 10);
    let (op1, ev1, _, _) = tc_support::record_dns_accepted(e, c, &dns1);

    let dec_a = terrain_support::allocate_op();
    let prep_a = m1_context::prepare(
        &mut PgContextReader::new(terrain_support::broker_client()),
        &q,
        dec_a,
    )
    .expect("prepare A");
    assert_eq!(prep_a.terrain_dependencies().len(), 1);
    assert_eq!(
        prep_a.terrain_dependencies()[0].claim_id,
        ev1.claim.claim_id
    );
    assert_eq!(prep_a.terrain_dependencies()[0].claim_revision, 1);

    // 2. TerrainBasis current-use check succeeds for Decision A while revision 1 is current
    let scope_a = Scope {
        engagement: e.0,
        campaign: c.0,
        decision: dec_a.0,
        mission_revision: 1,
        permission: q.mission.permission_id,
    };
    tc_support::terrain_support::native::bind_request(&serde_json::to_value(&q).unwrap(), &scope_a);
    let mut broker_a = duskweave::m1_reasoning::postgres::PgBroker {
        client: terrain_support::broker_client(),
        mission: duskweave::postgres_mission::PgMissionStore::new(terrain_support::runtime_client()),
        reservations: duskweave::postgres_inference_budget::PgInferenceReservationStore::new(
            terrain_support::broker_client(),
        ),
        request: q.mission.clone(),
        deadline: Instant::now() + Duration::from_secs(20),
    };
    let _res_a = broker_a
        .reserve(&scope_a, prep_a.utf8().as_bytes())
        .unwrap();
    let mut basis_a = TerrainBasis {
        mission: Basis {
            client: terrain_support::broker_client(),
            scope: scope_a.clone(),
            references: request::reference_ids(prep_a.utf8().as_bytes()).unwrap(),
            deadline: Instant::now() + Duration::from_secs(20),
        },
        dependencies: prep_a.terrain_dependencies().to_vec(),
    };
    assert_eq!(basis_a.before_provider_release(), Ok(()));
    assert_eq!(
        basis_a.current_authority_for_references(&["mission".into(), "terrain_0".into()]),
        Ok(())
    );

    // 3. Record Revision 2 on same applicability -> supersedes revision 1
    let mut dns2 = terrain_support::dns_request_json(e, c, now - 5);
    dns2["expected_claim_revision"] = json!(1);
    dns2["answers"] = json!([{"ipv4": "198.51.100.20", "ttl_seconds": 90}]);
    let (_, ev2, _, _) = tc_support::record_dns_accepted(e, c, &dns2);
    assert_eq!(ev2.claim.claim_revision, 2);

    // Old dependency (revision 1) is now rejected by TerrainBasis!
    assert_eq!(
        basis_a.before_provider_release(),
        Err("reference_ineligible")
    );
    assert_eq!(
        basis_a.current_authority_for_references(&["mission".into(), "terrain_0".into()]),
        Err("reference_ineligible")
    );
    // Uncited check (only citing mission and asset_0) still passes!
    assert_eq!(
        basis_a.current_authority_for_references(&["mission".into(), "asset_0".into()]),
        Ok(())
    );

    // Next ContextPack (Decision B) sees revision 2
    let prep_b = m1_context::prepare(
        &mut PgContextReader::new(terrain_support::broker_client()),
        &q,
        terrain_support::allocate_op(),
    )
    .expect("prepare B");
    assert_eq!(prep_b.terrain_dependencies().len(), 1);
    assert_eq!(prep_b.terrain_dependencies()[0].claim_revision, 2);

    // 4. Inject integrity hold on same applicability (conflicting replay of op1) -> removes premise from next pack
    let mut conflicting = dns1.clone();
    conflicting["perceived_at"] = json!(now - 2);
    conflicting["answers"] = json!([{"ipv4": "198.51.100.99", "ttl_seconds": 60}]);
    tc_support::inject_integrity_hold(e, c, op1, &conflicting);

    let prep_held = m1_context::prepare(
        &mut PgContextReader::new(terrain_support::broker_client()),
        &q,
        terrain_support::allocate_op(),
    )
    .expect("prepare held");
    assert!(prep_held.terrain_dependencies().is_empty());
    let held_pack: Value = serde_json::from_str(prep_held.utf8()).unwrap();
    assert_eq!(
        held_pack["owner_views"][1]["omissions"]["held_or_conflicted"],
        1
    );

    // 5. New qualified revision with perceived_at > hold_frontier restores only its own applicability
    let mut dns3 = terrain_support::dns_request_json(e, c, now - 1);
    dns3["expected_claim_revision"] = json!(2);
    dns3["answers"] = json!([{"ipv4": "198.51.100.30", "ttl_seconds": 60}]);
    let (_, ev3, _, _) = tc_support::record_dns_accepted(e, c, &dns3);
    assert_eq!(ev3.claim.claim_revision, 3);

    let prep_restored = m1_context::prepare(
        &mut PgContextReader::new(terrain_support::broker_client()),
        &q,
        terrain_support::allocate_op(),
    )
    .expect("prepare restored");
    assert_eq!(prep_restored.terrain_dependencies().len(), 1);
    assert_eq!(prep_restored.terrain_dependencies()[0].claim_revision, 3);

    // 6. Partial alias revision withholds the claim from active references
    let mut dns4 = terrain_support::dns_request_json(e, c, now);
    dns4["expected_claim_revision"] = json!(3);
    dns4["limitations"] = json!(["partial_alias_chain"]);
    let _ = tc_support::record_dns_accepted(e, c, &dns4);

    let prep_withheld = m1_context::prepare(
        &mut PgContextReader::new(terrain_support::broker_client()),
        &q,
        terrain_support::allocate_op(),
    )
    .expect("prepare withheld");
    assert!(prep_withheld.terrain_dependencies().is_empty());
    let withheld_pack: Value = serde_json::from_str(prep_withheld.utf8()).unwrap();
    assert_eq!(withheld_pack["owner_views"][1]["omissions"]["withheld"], 1);

    // 7. Mission withdrawal refuses direct owner view read
    let marker = terrain_support::policy_support::marker(&mission);
    terrain_support::policy_support::insert_marker(&mut terrain_support::runtime_client(), &marker);
    assert_eq!(
        postgres_terrain_view::read(
            &mut terrain_support::broker_client(),
            &mission,
            &[AssetRef(a1)],
            None
        )
        .err(),
        Some(Fail::State("terrain_context_scope_unavailable"))
    );
}

#[test]
fn incompatible_lineage_degrades_via_savepoint_and_claim_id_filter_prevents_starvation() {
    let _g = terrain_support::db();
    let (e, c, mission, raw_reg, now) = tc_support::register_combined_campaign(0xa103, true);
    let a1 = Uuid::from_u128(0x21);
    let a2 = Uuid::from_u128(0x22);
    let (_, q) = tc_support::context_request_for(
        e,
        c,
        &raw_reg,
        vec![(a1, Priority::High), (a2, Priority::Low)],
    );

    // Record 9 distinct DNS applicabilities across asset 0x22 (`*.other.invalid`)
    let mut recorded_claims = Vec::new();
    for idx in 0..9 {
        let mut dns = terrain_support::dns_request_json(e, c, now - 10);
        dns["asset_ref"] = json!(a2);
        dns["subject_name"] = json!(format!("host{idx}.other.invalid"));
        let (_, ev, _, _) = tc_support::record_dns_accepted(e, c, &dns);
        recorded_claims.push(ev.claim.claim_id);
    }
    recorded_claims.sort();
    let ninth_claim_id = *recorded_claims.last().unwrap();

    // Unfiltered read returns first 8 claims in claim_id ASC order and omitted_by_claim_limit = 1
    let mut broker = terrain_support::broker_client();
    let view8 = postgres_terrain_view::read(&mut broker, &mission, &[AssetRef(a2)], None).unwrap();
    assert_eq!(view8.claims.len(), 8);
    assert_eq!(view8.omitted_by_claim_limit, 1);
    assert!(!view8.claims.iter().any(|cl| cl.claim_id == ninth_claim_id));

    // Filtered read by exact claim_ids finds the 9th claim even when 8 earlier-sorting claims exist
    let view_ninth = postgres_terrain_view::read(
        &mut broker,
        &mission,
        &[AssetRef(a2)],
        Some(vec![ninth_claim_id]),
    )
    .unwrap();
    assert_eq!(view_ninth.claims.len(), 1);
    assert_eq!(view_ninth.claims[0].claim_id, ninth_claim_id);
    let dep9 = duskweave::m1_terrain_view::TerrainDependency::from_claim(
        "terrain_0".into(),
        &view_ninth.claims[0],
    )
    .unwrap();
    assert_eq!(
        verify_current_dependencies(&[dep9], &view_ninth, view_ninth.evaluated_at),
        Ok(())
    );

    // Corrupting stored lineage in terrain.current_claims triggers savepoint rollback in PgContextReader
    // and records availability="unavailable", reason="incompatible_lineage" without aborting the snapshot
    terrain_support::admin_client()
        .execute(
            "UPDATE terrain.current_claims SET payload = jsonb_set(payload, '{corroborated}', 'true'::jsonb) \
             WHERE engagement_id = $1 AND campaign_id = $2",
            &[&e.0, &c.0],
        )
        .unwrap();
    assert_eq!(
        postgres_terrain_view::read(&mut broker, &mission, &[AssetRef(a2)], None).err(),
        Some(Fail::State("incompatible_lineage"))
    );

    let prep_degraded = m1_context::prepare(
        &mut PgContextReader::new(terrain_support::broker_client()),
        &q,
        OperationId(Uuid::from_u128(0x00a1_0399)),
    )
    .expect("PgContextReader savepoint must recover from incompatible_lineage");
    assert!(prep_degraded.terrain_dependencies().is_empty());
    let pack_degraded: Value = serde_json::from_str(prep_degraded.utf8()).unwrap();
    assert_eq!(
        pack_degraded["owner_views"][1]["availability"],
        "unavailable"
    );
    assert_eq!(
        pack_degraded["owner_views"][1]["reason"],
        "incompatible_lineage"
    );
}

struct BarrierProvider<F: FnMut(&mut dyn ProposalBasis)> {
    prepared: usize,
    sent: usize,
    cite_refs: Vec<String>,
    on_call: F,
}

impl<F: FnMut(&mut dyn ProposalBasis)> Provider for BarrierProvider<F> {
    fn prepare(&mut self, _: &BoundCall, _: Instant) -> Result<(), &'static str> {
        self.prepared += 1;
        Ok(())
    }

    fn call(
        &mut self,
        _: &BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        release()?;
        self.sent += 1;
        (self.on_call)(basis);
        let proposal = json!({
            "version": "m1_inference_proposal_v1",
            "hypothesis": "Terrain-qualified DNS summary supports candidate access validation",
            "considered_alternative": "Defer candidate selection until additional observation",
            "references": self.cite_refs,
            "expected_observation": "Consistent HTTPS reachability under permitted envelope",
            "falsifying_observation": "Applicability conflict or authority withdrawal",
            "uncertainty": "Bounded single-vantage TTL window",
            "rationale": "Owner-qualified DNS summary is fresh and unconflicted",
            "next_intent": {
                "category": "observe_more",
                "references": self.cite_refs,
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
            "usage": {"input_tokens": 12, "output_tokens": 20}
        });
        response::admit_envelope(
            &serde_json::to_vec(&envelope).unwrap(),
            100,
            Duration::ZERO,
            basis,
        )
    }
}

#[test]
fn barrier_races_before_release_and_during_transfer_for_terrain_and_withdrawal() {
    let _g = terrain_support::db();
    let a1 = Uuid::from_u128(0x21);

    // 1. Changed Terrain premise BEFORE release sends zero requests (prepare alone is not a send)
    {
        let (e, c, _, raw_reg, now) = tc_support::register_combined_campaign(0xa201, true);
        let dns1 = terrain_support::dns_request_json(e, c, now - 10);
        let _ = tc_support::record_dns_accepted(e, c, &dns1);
        let (req_val, q) =
            tc_support::context_request_for(e, c, &raw_reg, vec![(a1, Priority::High)]);
        let dec = terrain_support::allocate_op();
        let scope = Scope {
            engagement: e.0,
            campaign: c.0,
            decision: dec.0,
            mission_revision: 1,
            permission: q.mission.permission_id,
        };
        let input_hash = terrain_support::native::bind_request(&req_val, &scope);
        let prep = m1_context::prepare(
            &mut PgContextReader::new(terrain_support::broker_client()),
            &q,
            dec,
        )
        .unwrap();
        assert_eq!(prep.terrain_dependencies().len(), 1);

        // Supersede terrain_0 BEFORE release_into
        let mut dns2 = terrain_support::dns_request_json(e, c, now - 5);
        dns2["expected_claim_revision"] = json!(1);
        dns2["answers"] = json!([{"ipv4": "198.51.100.20", "ttl_seconds": 60}]);
        let _ = tc_support::record_dns_accepted(e, c, &dns2);

        let mut broker = PgBroker {
            client: terrain_support::broker_client(),
            mission: duskweave::postgres_mission::PgMissionStore::new(
                terrain_support::runtime_client(),
            ),
            reservations: duskweave::postgres_inference_budget::PgInferenceReservationStore::new(
                terrain_support::broker_client(),
            ),
            request: q.mission.clone(),
            deadline: Instant::now() + Duration::from_secs(20),
        };
        let mut store = PgDecisionStore {
            client: terrain_support::cognition_client(),
            input_hash,
        };
        let mut history =
            duskweave::postgres_trajectory::PgTrajectory::new(terrain_support::trajectory_client());
        let mut basis = TerrainBasis {
            mission: Basis {
                client: terrain_support::broker_client(),
                scope: scope.clone(),
                references: request::reference_ids(prep.utf8().as_bytes()).unwrap(),
                deadline: broker.deadline,
            },
            dependencies: prep.terrain_dependencies().to_vec(),
        };
        let mut provider = BarrierProvider {
            prepared: 0,
            sent: 0,
            cite_refs: vec!["mission".into(), "asset_0".into(), "terrain_0".into()],
            on_call: |_| {
                panic!("provider call must not execute when before_provider_release fails")
            },
        };
        let receipt = inference::infer(
            &mut broker,
            &mut provider,
            &mut basis,
            &mut store,
            &mut history,
            InferenceInput {
                scope: &scope,
                pack: prep.utf8().as_bytes(),
                event_id: terrain_support::allocate_op().0,
                occurred_at_ms: (now as u64) * 1000,
                source_mode: "blind".into(),
            },
        );
        assert_eq!(provider.prepared, 1);
        assert_eq!(provider.sent, 0);
        assert_eq!(receipt.proposal_disposition, "reference_ineligible");
        assert_eq!(receipt.consumption, "failed_never_released_reserved");
    }

    // 2. Cited Terrain premise invalidated DURING transfer is rejected, while uncited change permits supported proposal
    {
        let (e, c, _, raw_reg, now) = tc_support::register_combined_campaign(0xa202, true);
        let dns1 = terrain_support::dns_request_json(e, c, now - 10);
        let _ = tc_support::record_dns_accepted(e, c, &dns1);
        let (req_val, q) =
            tc_support::context_request_for(e, c, &raw_reg, vec![(a1, Priority::High)]);
        let dec = terrain_support::allocate_op();
        let scope = Scope {
            engagement: e.0,
            campaign: c.0,
            decision: dec.0,
            mission_revision: 1,
            permission: q.mission.permission_id,
        };
        let input_hash = terrain_support::native::bind_request(&req_val, &scope);
        let prep = m1_context::prepare(
            &mut PgContextReader::new(terrain_support::broker_client()),
            &q,
            dec,
        )
        .unwrap();

        let mut broker = PgBroker {
            client: terrain_support::broker_client(),
            mission: duskweave::postgres_mission::PgMissionStore::new(
                terrain_support::runtime_client(),
            ),
            reservations: duskweave::postgres_inference_budget::PgInferenceReservationStore::new(
                terrain_support::broker_client(),
            ),
            request: q.mission.clone(),
            deadline: Instant::now() + Duration::from_secs(20),
        };
        let mut store = PgDecisionStore {
            client: terrain_support::cognition_client(),
            input_hash,
        };
        let mut history =
            duskweave::postgres_trajectory::PgTrajectory::new(terrain_support::trajectory_client());
        let mut basis = TerrainBasis {
            mission: Basis {
                client: terrain_support::broker_client(),
                scope: scope.clone(),
                references: request::reference_ids(prep.utf8().as_bytes()).unwrap(),
                deadline: broker.deadline,
            },
            dependencies: prep.terrain_dependencies().to_vec(),
        };
        let mut provider = BarrierProvider {
            prepared: 0,
            sent: 0,
            cite_refs: vec!["mission".into(), "asset_0".into(), "terrain_0".into()],
            on_call: |_| {
                let mut dns2 = terrain_support::dns_request_json(e, c, now - 5);
                dns2["expected_claim_revision"] = json!(1);
                dns2["answers"] = json!([{"ipv4": "198.51.100.25", "ttl_seconds": 60}]);
                let _ = tc_support::record_dns_accepted(e, c, &dns2);
            },
        };
        let receipt = inference::infer(
            &mut broker,
            &mut provider,
            &mut basis,
            &mut store,
            &mut history,
            InferenceInput {
                scope: &scope,
                pack: prep.utf8().as_bytes(),
                event_id: terrain_support::allocate_op().0,
                occurred_at_ms: (now as u64) * 1000,
                source_mode: "blind".into(),
            },
        );
        assert_eq!(provider.sent, 1);
        assert_eq!(receipt.proposal_disposition, "reference_ineligible");
        assert!(receipt.proposal.is_none());
    }

    // 3. Uncited Terrain change during transfer: transport progress hook (current_authority_and_premises)
    // remains permitted under current Mission, and proposal citing only still-valid references is accepted
    {
        let (e, c, _, raw_reg, now) = tc_support::register_combined_campaign(0xa203, true);
        let dns1 = terrain_support::dns_request_json(e, c, now - 10);
        let _ = tc_support::record_dns_accepted(e, c, &dns1);
        let (req_val, q) =
            tc_support::context_request_for(e, c, &raw_reg, vec![(a1, Priority::High)]);
        let dec = terrain_support::allocate_op();
        let scope = Scope {
            engagement: e.0,
            campaign: c.0,
            decision: dec.0,
            mission_revision: 1,
            permission: q.mission.permission_id,
        };
        let input_hash = terrain_support::native::bind_request(&req_val, &scope);
        let prep = m1_context::prepare(
            &mut PgContextReader::new(terrain_support::broker_client()),
            &q,
            dec,
        )
        .unwrap();

        let mut broker = PgBroker {
            client: terrain_support::broker_client(),
            mission: duskweave::postgres_mission::PgMissionStore::new(
                terrain_support::runtime_client(),
            ),
            reservations: duskweave::postgres_inference_budget::PgInferenceReservationStore::new(
                terrain_support::broker_client(),
            ),
            request: q.mission.clone(),
            deadline: Instant::now() + Duration::from_secs(20),
        };
        let mut store = PgDecisionStore {
            client: terrain_support::cognition_client(),
            input_hash,
        };
        let mut history =
            duskweave::postgres_trajectory::PgTrajectory::new(terrain_support::trajectory_client());
        let mut basis = TerrainBasis {
            mission: Basis {
                client: terrain_support::broker_client(),
                scope: scope.clone(),
                references: request::reference_ids(prep.utf8().as_bytes()).unwrap(),
                deadline: broker.deadline,
            },
            dependencies: prep.terrain_dependencies().to_vec(),
        };
        let mut provider = BarrierProvider {
            prepared: 0,
            sent: 0,
            cite_refs: vec!["mission".into(), "asset_0".into()],
            on_call: |b| {
                // Supersede uncited terrain_0 during transfer AND record another independent claim
                let mut dns2 = terrain_support::dns_request_json(e, c, now - 5);
                dns2["expected_claim_revision"] = json!(1);
                dns2["answers"] = json!([{"ipv4": "198.51.100.28", "ttl_seconds": 60}]);
                let _ = tc_support::record_dns_accepted(e, c, &dns2);
                // Invoke transport progress hook: must still pass under current Mission!
                assert_eq!(b.current_authority_and_premises(), Ok(()));
            },
        };
        let receipt = inference::infer(
            &mut broker,
            &mut provider,
            &mut basis,
            &mut store,
            &mut history,
            InferenceInput {
                scope: &scope,
                pack: prep.utf8().as_bytes(),
                event_id: terrain_support::allocate_op().0,
                occurred_at_ms: (now as u64) * 1000,
                source_mode: "blind".into(),
            },
        );
        assert_eq!(provider.sent, 1);
        assert_eq!(receipt.proposal_disposition, "validated_proposal");
        assert!(receipt.proposal.is_some());
    }

    // 4a. Mission withdrawal BEFORE release (after reservation, before release_into -> sends zero requests)
    {
        let (e, c, mission, raw_reg, now) = tc_support::register_combined_campaign(0xa204, true);
        let dns1 = terrain_support::dns_request_json(e, c, now - 10);
        let _ = tc_support::record_dns_accepted(e, c, &dns1);
        let (req_val, q) =
            tc_support::context_request_for(e, c, &raw_reg, vec![(a1, Priority::High)]);
        let dec = terrain_support::allocate_op();
        let scope = Scope {
            engagement: e.0,
            campaign: c.0,
            decision: dec.0,
            mission_revision: 1,
            permission: q.mission.permission_id,
        };
        let input_hash = terrain_support::native::bind_request(&req_val, &scope);
        let prep = m1_context::prepare(
            &mut PgContextReader::new(terrain_support::broker_client()),
            &q,
            dec,
        )
        .unwrap();

        let mut broker = PgBroker {
            client: terrain_support::broker_client(),
            mission: duskweave::postgres_mission::PgMissionStore::new(
                terrain_support::runtime_client(),
            ),
            reservations: duskweave::postgres_inference_budget::PgInferenceReservationStore::new(
                terrain_support::broker_client(),
            ),
            request: q.mission.clone(),
            deadline: Instant::now() + Duration::from_secs(20),
        };
        let mut store = PgDecisionStore {
            client: terrain_support::cognition_client(),
            input_hash,
        };
        let mut history =
            duskweave::postgres_trajectory::PgTrajectory::new(terrain_support::trajectory_client());
        let mut basis = TerrainBasis {
            mission: Basis {
                client: terrain_support::broker_client(),
                scope: scope.clone(),
                references: request::reference_ids(prep.utf8().as_bytes()).unwrap(),
                deadline: broker.deadline,
            },
            dependencies: prep.terrain_dependencies().to_vec(),
        };
        // Pre-reserve while Mission is active, then withdraw Mission before infer/may_send/release
        let _ = broker.reserve(&scope, prep.utf8().as_bytes()).unwrap();
        let marker = terrain_support::policy_support::marker(&mission);
        terrain_support::policy_support::insert_marker(
            &mut terrain_support::runtime_client(),
            &marker,
        );
        let mut provider = BarrierProvider {
            prepared: 0,
            sent: 0,
            cite_refs: vec!["mission".into(), "asset_0".into(), "terrain_0".into()],
            on_call: |_| panic!("withdrawn mission before release must never call provider"),
        };
        let receipt = inference::infer(
            &mut broker,
            &mut provider,
            &mut basis,
            &mut store,
            &mut history,
            InferenceInput {
                scope: &scope,
                pack: prep.utf8().as_bytes(),
                event_id: terrain_support::allocate_op().0,
                occurred_at_ms: (now as u64) * 1000,
                source_mode: "blind".into(),
            },
        );
        assert_eq!(provider.prepared, 1);
        assert_eq!(provider.sent, 0);
        assert!(receipt.proposal.is_none());
    }

    // 4b. Mission withdrawal AFTER send (during transfer -> rejected at admission)
    {
        let (e, c, mission, raw_reg, now) = tc_support::register_combined_campaign(0xa205, true);
        let dns1 = terrain_support::dns_request_json(e, c, now - 10);
        let _ = tc_support::record_dns_accepted(e, c, &dns1);
        let (req_val, q) =
            tc_support::context_request_for(e, c, &raw_reg, vec![(a1, Priority::High)]);
        let dec = terrain_support::allocate_op();
        let scope = Scope {
            engagement: e.0,
            campaign: c.0,
            decision: dec.0,
            mission_revision: 1,
            permission: q.mission.permission_id,
        };
        let input_hash = terrain_support::native::bind_request(&req_val, &scope);
        let prep = m1_context::prepare(
            &mut PgContextReader::new(terrain_support::broker_client()),
            &q,
            dec,
        )
        .unwrap();

        let mut broker = PgBroker {
            client: terrain_support::broker_client(),
            mission: duskweave::postgres_mission::PgMissionStore::new(
                terrain_support::runtime_client(),
            ),
            reservations: duskweave::postgres_inference_budget::PgInferenceReservationStore::new(
                terrain_support::broker_client(),
            ),
            request: q.mission.clone(),
            deadline: Instant::now() + Duration::from_secs(20),
        };
        let mut store = PgDecisionStore {
            client: terrain_support::cognition_client(),
            input_hash,
        };
        let mut history =
            duskweave::postgres_trajectory::PgTrajectory::new(terrain_support::trajectory_client());
        let mut basis = TerrainBasis {
            mission: Basis {
                client: terrain_support::broker_client(),
                scope: scope.clone(),
                references: request::reference_ids(prep.utf8().as_bytes()).unwrap(),
                deadline: broker.deadline,
            },
            dependencies: prep.terrain_dependencies().to_vec(),
        };
        let mut provider = BarrierProvider {
            prepared: 0,
            sent: 0,
            cite_refs: vec!["mission".into(), "asset_0".into(), "terrain_0".into()],
            on_call: |b| {
                let marker = terrain_support::policy_support::marker(&mission);
                terrain_support::policy_support::insert_marker(
                    &mut terrain_support::runtime_client(),
                    &marker,
                );
                assert_eq!(
                    b.current_authority_and_premises(),
                    Err("current_authority_unavailable")
                );
            },
        };
        let receipt = inference::infer(
            &mut broker,
            &mut provider,
            &mut basis,
            &mut store,
            &mut history,
            InferenceInput {
                scope: &scope,
                pack: prep.utf8().as_bytes(),
                event_id: terrain_support::allocate_op().0,
                occurred_at_ms: (now as u64) * 1000,
                source_mode: "blind".into(),
            },
        );
        assert_eq!(provider.sent, 1);
        assert_eq!(
            receipt.proposal_disposition,
            "current_authority_unavailable"
        );
        assert!(receipt.proposal.is_none());
    }
}
