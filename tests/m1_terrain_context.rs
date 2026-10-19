//! Pure unit and invariant tests for Terrain public view, ContextPack assembly, byte fitting, and dependency admission.
use duskweave::Fail;
use duskweave::m1_context::{self, ContextBasis, ContextReader, TerrainContextState};
use duskweave::m1_context_capacity::ContextCapacity;
use duskweave::m1_context_input::{Candidate, ContextRequest, Priority};
use duskweave::m1_inference_budget::{ReservationRequest, ReservedCapacity};
use duskweave::m1_reasoning::{
    proposal::{self, Admission, ProposalBasis},
    request,
};
use duskweave::m1_terrain::{
    CLAIM_KIND_DNS_IPV4, DnsCompleteness, EvaluationOutcome, TerrainClaimStatus, TerrainIds,
    TerrainSourceKind, TrustTier,
};
use duskweave::m1_terrain_input::DnsLimitation;
use duskweave::m1_terrain_view::{
    RawTerrainClaim, RawTerrainView, TerrainDependency, TerrainPublicView, TerrainViewQuery,
    VIEW_VERSION, verify_current_dependencies,
};
use duskweave::mission::{
    AssetRef, CONTRACT_KIND, CampaignId, EngagementId, ExerciseMode, GoalRef, MissionRegistered,
    OperationId, OperatorRef, PRODUCER,
};
use serde_json::{Value, json};
use uuid::Uuid;

struct FixedReader(ContextBasis);
impl ContextReader for FixedReader {
    fn read(&mut self, _q: &ContextRequest, _op: OperationId) -> duskweave::Res<ContextBasis> {
        Ok(ContextBasis {
            mission: self.0.mission.clone(),
            capacity: ContextCapacity {
                version: self.0.capacity.version.clone(),
                epoch: self.0.capacity.epoch,
                session_claimed: self.0.capacity.session_claimed,
                active_holds: self.0.capacity.active_holds,
                charged: self.0.capacity.charged.clone(),
            },
            evaluated_at: self.0.evaluated_at,
            terrain: self.0.terrain.clone(),
        })
    }
}

fn build_mission(
    e: EngagementId,
    c: CampaignId,
    now: i64,
    with_summaries: bool,
    context_bytes: usize,
) -> (MissionRegistered, ReservationRequest) {
    let perm_id = Uuid::from_u128(0x3100);
    let mut classes = vec![
        "mission_question_constraints",
        "scoped_opaque_references",
        "omissions_unknown",
        "capacity",
        "admitted_operator_premises",
        "admitted_repair_fields_codes",
    ];
    if with_summaries {
        classes.push("owner_qualified_summaries");
    }
    let reg_op = Uuid::from_u128(0xa002);
    let reg_ev = Uuid::from_u128(0xa001);
    let m1_perm = json!({
        "policy_version": 1,
        "ct_base_domain": "example.invalid",
        "provider_disclosure": "crt_sh",
        "vantage_ref": Uuid::from_u128(0x31),
        "resolver_ipv4": "192.0.2.53",
        "discovery_rules": [{"label_suffix": "example.invalid"}],
        "contact_rules": [
            {"asset_ref": Uuid::from_u128(0x21), "rule": {"exact": "www.example.invalid"}, "priority": 1},
            {"asset_ref": Uuid::from_u128(0x22), "rule": {"label_suffix": "other.invalid"}, "priority": 2},
            {"asset_ref": Uuid::from_u128(0x23), "rule": {"label_suffix": "third.invalid"}, "priority": 3}
        ],
        "excluded_names": [{"label_suffix": "excluded.example.invalid"}],
        "approved_path": "/",
        "starts_at": now - 1800,
        "ends_at": now + 1800,
        "campaign_limits": {
            "episodes": 2,
            "provider_calls": 3,
            "dns_questions": 10,
            "dns_followups": 2,
            "tcp_connections": 5,
            "head_requests": 2
        },
        "concurrency": 1
    });
    let op_limits = json!({
        "context_bytes": context_bytes,
        "candidates": 8,
        "request_bytes": 16384,
        "configuration_bytes": 16384,
        "response_bytes": 65536,
        "calls_per_decision": 2,
        "output_tokens_per_call": 4096,
        "decision_wall_time_ms": 20000,
        "connect_time_ms": 5000,
        "proposal_bytes": 8192,
        "repair_bytes": 2048,
        "text_field_bytes": 1024,
        "repair_error_codes": 8,
        "json_depth": 32,
        "collection_entries": 64,
        "concurrency": 1
    });
    let inf_perm = json!({
        "version": "m1_inference_permission_v2",
        "permission_id": perm_id,
        "engagement_id": e,
        "campaign_id": c,
        "operator_ref": Uuid::from_u128(0x11),
        "authority_ref": Uuid::from_u128(0x12),
        "authority_revision": 17,
        "mission_revision": 1,
        "goal_ref": Uuid::from_u128(0x13),
        "exercise_mode": "blind",
        "recipient": "https://api.deepseek.com/responses",
        "profile": "deepseek-responses-envelope32-v1",
        "field_classes": classes,
        "starts_at": now - 1800,
        "ends_at": now + 1800,
        "provider_terms_ref": Uuid::from_u128(0x32),
        "provider_terms_approved": true,
        "campaign_limits": {
            "calls": 10,
            "request_bytes": 100000,
            "response_bytes": 500000,
            "output_token_capacity": 20000,
            "wall_time_ms": 100000
        },
        "operation_limits": op_limits
    });
    let fields = json!({
        "engagement_id": e,
        "campaign_id": c,
        "operator_ref": Uuid::from_u128(0x11),
        "authority_ref": Uuid::from_u128(0x12),
        "authority_revision": 17,
        "goal_ref": Uuid::from_u128(0x13),
        "included_assets": [Uuid::from_u128(0x21), Uuid::from_u128(0x22), Uuid::from_u128(0x23)],
        "excluded_assets": [Uuid::from_u128(0x29)],
        "exercise_mode": "blind",
        "starts_at": now - 3600,
        "ends_at": now + 3600,
        "m1_permission": m1_perm,
        "m1_inference_permission": inf_perm
    });
    let mission: MissionRegistered = serde_json::from_value(json!({
        "event_id": reg_ev,
        "operation_id": reg_op,
        "engagement_id": e,
        "campaign_id": c,
        "producer": PRODUCER,
        "affected_entity": c,
        "owner_revision": 1,
        "kind": CONTRACT_KIND,
        "version": 3,
        "causation_id": reg_op,
        "correlation_id": reg_op,
        "occurred_at": now,
        "recorded_at": now,
        "fields": fields
    }))
    .unwrap();
    let req = ReservationRequest {
        version: "m1_inference_reservation_request_v1".into(),
        engagement_id: e,
        campaign_id: c,
        operator_ref: OperatorRef(Uuid::from_u128(0x11)),
        expected_mission_revision: 1,
        permission_id: perm_id,
        goal_ref: GoalRef(Uuid::from_u128(0x13)),
        exercise_mode: ExerciseMode::Blind,
    };
    (mission, req)
}

fn default_capacity() -> ContextCapacity {
    ContextCapacity {
        version: "m1_inference_capacity_snapshot_v1".into(),
        epoch: 3,
        session_claimed: false,
        active_holds: 0,
        charged: ReservedCapacity {
            calls: 0,
            request_bytes: 0,
            response_bytes: 0,
            output_token_capacity: 0,
            wall_time_ms: 0,
        },
    }
}

fn sample_raw_claim(
    e: EngagementId,
    c: CampaignId,
    op_seed: u128,
    asset: u128,
    perceived_at: i64,
) -> RawTerrainClaim {
    let op = OperationId(Uuid::from_u128(op_seed));
    let ids = TerrainIds::derive(e, c, op).unwrap();
    RawTerrainClaim {
        claim_id: ids.claim_id,
        claim_revision: 1,
        previous_claim_revision: 0,
        operation_id: op,
        event_id: ids.event_id,
        observation_id: ids.observation_id,
        material_id: ids.material_id,
        evaluation_id: ids.evaluation_id,
        asset_ref: AssetRef(Uuid::from_u128(asset)),
        vantage_ref: Uuid::from_u128(0x31),
        source_kind: TerrainSourceKind::OperatorReviewedDns,
        claim_kind: CLAIM_KIND_DNS_IPV4.to_string(),
        trust_tier: TrustTier::Tier1,
        claim_status: TerrainClaimStatus::Observed,
        corroborated: false,
        conflicted: false,
        integrity_hold: false,
        hold_frontier: None,
        temporal_frontier: perceived_at,
        perceived_at,
        uncertainty_seconds: 1,
        valid_until: Some(perceived_at + 60),
        lowest_ttl_seconds: Some(120),
        answer_count: 1,
        completeness: DnsCompleteness::Complete,
        limitations: vec![DnsLimitation::SingleVantageOnly],
        outcome: EvaluationOutcome::QualifiedDnsClaim,
    }
}

fn build_view(
    mission: &MissionRegistered,
    selected_assets: Vec<AssetRef>,
    evaluated_at: i64,
    mut claims: Vec<RawTerrainClaim>,
    omitted_by_claim_limit: u32,
) -> TerrainPublicView {
    claims.sort_by_key(|c| c.claim_id);
    let raw = RawTerrainView {
        version: VIEW_VERSION.to_string(),
        engagement_id: mission.engagement_id,
        campaign_id: mission.campaign_id,
        mission_revision: 1,
        registration_operation_id: mission.operation_id,
        registration_event_id: mission.event_id,
        vantage_ref: Uuid::from_u128(0x31),
        evaluated_at,
        selected_assets: selected_assets.clone(),
        claims,
        omitted_by_claim_limit,
    };
    TerrainPublicView::from_raw(raw, mission, &selected_assets).unwrap()
}

#[test]
fn disclosure_not_permitted_and_not_requested_and_unavailable_states_are_distinct() {
    let now = 1_700_000_000;
    let (e, c) = (
        EngagementId(Uuid::from_u128(0x101)),
        CampaignId(Uuid::from_u128(0x201)),
    );
    let a1 = AssetRef(Uuid::from_u128(0x21));
    let op = OperationId(Uuid::from_u128(0x901));

    // 1. Without owner_qualified_summaries in field_classes -> disclosure_not_permitted even if view supplied
    let (m_no_sum, req_no_sum) = build_mission(e, c, now, false, 4096);
    let view = build_view(
        &m_no_sum,
        vec![a1],
        now,
        vec![sample_raw_claim(e, c, 0x501, 0x21, now - 10)],
        0,
    );
    let mut reader_no_sum = FixedReader(ContextBasis {
        mission: m_no_sum,
        capacity: default_capacity(),
        evaluated_at: now,
        terrain: TerrainContextState::Available(view),
    });
    let q_cand = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req_no_sum,
        candidates: vec![Candidate {
            asset_ref: a1,
            priority: Priority::High,
        }],
    };
    let prep = m1_context::prepare(&mut reader_no_sum, &q_cand, op).unwrap();
    assert!(prep.terrain_dependencies().is_empty());
    let pack: Value = serde_json::from_str(prep.utf8()).unwrap();
    assert_eq!(pack["owner_views"][1]["owner"], "cyber_terrain");
    assert_eq!(pack["owner_views"][1]["availability"], "unavailable");
    assert_eq!(pack["owner_views"][1]["reason"], "disclosure_not_permitted");
    assert_eq!(pack["references"].as_array().unwrap().len(), 2);

    // 2. With owner_qualified_summaries but no candidates -> not_requested
    let (m_sum, req_sum) = build_mission(e, c, now, true, 4096);
    let mut reader_not_req = FixedReader(ContextBasis {
        mission: m_sum.clone(),
        capacity: default_capacity(),
        evaluated_at: now,
        terrain: TerrainContextState::NotRequested,
    });
    let q_empty = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req_sum.clone(),
        candidates: vec![],
    };
    let prep_empty = m1_context::prepare(&mut reader_not_req, &q_empty, op).unwrap();
    let pack_empty: Value = serde_json::from_str(prep_empty.utf8()).unwrap();
    assert_eq!(pack_empty["owner_views"][1]["availability"], "unavailable");
    assert_eq!(pack_empty["owner_views"][1]["reason"], "not_requested");

    // 3. Explicit Unavailable("owner_view_unavailable") and Unavailable("incompatible_lineage")
    for reason in ["owner_view_unavailable", "incompatible_lineage"] {
        let mut reader_unavail = FixedReader(ContextBasis {
            mission: m_sum.clone(),
            capacity: default_capacity(),
            evaluated_at: now,
            terrain: TerrainContextState::Unavailable(reason),
        });
        let q_with_cand = ContextRequest {
            version: "m1_context_prepare_request_v1".into(),
            mission: req_sum.clone(),
            candidates: vec![Candidate {
                asset_ref: a1,
                priority: Priority::High,
            }],
        };
        let prep_unavail = m1_context::prepare(&mut reader_unavail, &q_with_cand, op).unwrap();
        let pack_unavail: Value = serde_json::from_str(prep_unavail.utf8()).unwrap();
        assert_eq!(
            pack_unavail["owner_views"][1]["availability"],
            "unavailable"
        );
        assert_eq!(pack_unavail["owner_views"][1]["reason"], reason);
        assert!(prep_unavail.terrain_dependencies().is_empty());
    }
}

#[test]
fn positive_terrain_projection_links_candidates_and_excludes_sensitive_material() {
    let now = 1_700_000_000;
    let (e, c) = (
        EngagementId(Uuid::from_u128(0x102)),
        CampaignId(Uuid::from_u128(0x202)),
    );
    let a1 = AssetRef(Uuid::from_u128(0x21));
    let op = OperationId(Uuid::from_u128(0x902));
    let (mission, req) = build_mission(e, c, now, true, 4096);

    let raw_claim = sample_raw_claim(e, c, 0x502, 0x21, now - 15);
    let view = build_view(&mission, vec![a1], now, vec![raw_claim.clone()], 0);

    let mut reader = FixedReader(ContextBasis {
        mission,
        capacity: default_capacity(),
        evaluated_at: now,
        terrain: TerrainContextState::Available(view),
    });
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req,
        candidates: vec![Candidate {
            asset_ref: a1,
            priority: Priority::High,
        }],
    };

    let prep = m1_context::prepare(&mut reader, &q, op).unwrap();
    let utf8 = prep.utf8();
    assert!(utf8.len() <= 4096, "pack bytes {} <= 4096", utf8.len());
    // Must never contain hostname or IPv4 addresses
    for forbidden in ["www.example.invalid", "192.0.2.53", "198.51.100.10"] {
        assert!(!utf8.contains(forbidden), "forbidden leak: {forbidden}");
    }

    // Must pass bounded_json parser and request::reference_ids
    let ref_ids = request::reference_ids(utf8.as_bytes()).expect("valid bounded JSON pack");
    assert_eq!(ref_ids, vec!["mission", "asset_0", "terrain_0"]);

    let deps = prep.terrain_dependencies();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].local_id, "terrain_0");
    assert_eq!(deps[0].claim_id, raw_claim.claim_id);
    assert_eq!(deps[0].claim_revision, 1);
    assert_eq!(deps[0].asset_ref, a1);
    assert_eq!(deps[0].material_id, raw_claim.material_id);
    assert_eq!(deps[0].evaluation_id, raw_claim.evaluation_id);
    assert_eq!(deps[0].observation_id, raw_claim.observation_id);
    assert_eq!(deps[0].event_id, raw_claim.event_id);
    assert_eq!(deps[0].operation_id, raw_claim.operation_id);

    let pack: Value = serde_json::from_str(utf8).unwrap();
    assert_eq!(pack["owner_views"][1]["owner"], "cyber_terrain");
    assert_eq!(pack["owner_views"][1]["availability"], "available");
    assert_eq!(pack["owner_views"][1]["source"], "operator_reviewed_dns");
    assert_eq!(
        pack["owner_views"][1]["claim_kind"],
        "dns_hostname_ipv4_resolution"
    );
    assert_eq!(pack["owner_views"][1]["shown_claims"], 1);
    assert_eq!(
        pack["candidates"][0]["terrain_references"],
        json!(["terrain_0"])
    );

    let t_ref = &pack["references"][2];
    assert_eq!(t_ref["id"], "terrain_0");
    assert_eq!(t_ref["owner"], "cyber_terrain");
    assert_eq!(t_ref["entity"], json!(raw_claim.claim_id));
    assert_eq!(t_ref["asset_ref"], json!(a1.0));
    assert_eq!(t_ref["revision"], 1);
    assert_eq!(t_ref["status"], "observed");
    assert_eq!(t_ref["tier"], "tier1");
    assert_eq!(t_ref["freshness"], "fresh");
    assert_eq!(t_ref["completeness"], "complete");
    assert_eq!(t_ref["corroborated"], false);
    assert_eq!(t_ref["conflicted"], false);
    assert_eq!(t_ref["integrity_hold"], false);
}

#[test]
fn expired_withheld_conflicted_and_held_claims_are_excluded_from_active_references() {
    let now = 1_700_000_000;
    let (e, c) = (
        EngagementId(Uuid::from_u128(0x103)),
        CampaignId(Uuid::from_u128(0x203)),
    );
    let a1 = AssetRef(Uuid::from_u128(0x21));
    let op = OperationId(Uuid::from_u128(0x903));
    let (mission, req) = build_mission(e, c, now, true, 4096);

    // 1. Expired observed claim (perceived_at = now - 120, valid_until = now - 60)
    let expired = sample_raw_claim(e, c, 0x601, 0x21, now - 120);
    // 2. Withheld partial alias claim
    let mut withheld = sample_raw_claim(e, c, 0x602, 0x21, now - 10);
    withheld.claim_status = TerrainClaimStatus::Withheld;
    withheld.outcome = EvaluationOutcome::IncompleteMaterial;
    withheld.limitations = vec![DnsLimitation::PartialAliasChain];
    withheld.valid_until = None;
    withheld.lowest_ttl_seconds = None;
    withheld.answer_count = 0;
    // 3. Conflicted applicability claim
    let mut conflicted = sample_raw_claim(e, c, 0x603, 0x21, now - 10);
    conflicted.claim_status = TerrainClaimStatus::Conflicted;
    conflicted.conflicted = true;
    conflicted.outcome = EvaluationOutcome::ApplicabilityConflict;
    conflicted.valid_until = None;
    conflicted.lowest_ttl_seconds = None;
    // 4. Held observed claim (integrity_hold = true)
    let mut held = sample_raw_claim(e, c, 0x604, 0x21, now - 10);
    held.integrity_hold = true;
    held.hold_frontier = Some(now - 10);

    let view = build_view(
        &mission,
        vec![a1],
        now,
        vec![expired, withheld, conflicted, held],
        2,
    );
    let mut reader = FixedReader(ContextBasis {
        mission,
        capacity: default_capacity(),
        evaluated_at: now,
        terrain: TerrainContextState::Available(view),
    });
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req,
        candidates: vec![Candidate {
            asset_ref: a1,
            priority: Priority::High,
        }],
    };
    let prep = m1_context::prepare(&mut reader, &q, op).unwrap();
    assert!(prep.terrain_dependencies().is_empty());

    let pack: Value = serde_json::from_str(prep.utf8()).unwrap();
    let tv = &pack["owner_views"][1];
    assert_eq!(tv["availability"], "available");
    assert_eq!(tv["shown_claims"], 0);
    assert_eq!(tv["hold_frontier"], now - 10);
    assert_eq!(tv["omissions"]["stale_expired"], 1);
    assert_eq!(tv["omissions"]["withheld"], 1);
    assert_eq!(tv["omissions"]["held_or_conflicted"], 2);
    assert_eq!(tv["omissions"]["omitted_by_claim_limit"], 2);
    assert_eq!(tv["omissions"]["omitted_by_byte_bound"], 0);
    assert_eq!(pack["references"].as_array().unwrap().len(), 2);
}

#[test]
fn forged_terrain_summaries_and_invalid_queries_fail_closed() {
    let now = 1_700_000_000;
    let (e, c) = (
        EngagementId(Uuid::from_u128(0x104)),
        CampaignId(Uuid::from_u128(0x204)),
    );
    let a1 = AssetRef(Uuid::from_u128(0x21));
    let (mission, req) = build_mission(e, c, now, true, 4096);

    // Invalid TerrainViewQuery bounds
    assert!(TerrainViewQuery::from_mission(&mission, vec![], None).is_err());
    assert!(TerrainViewQuery::from_mission(&mission, vec![a1, a1], None).is_err());
    let nine_assets: Vec<AssetRef> = (1..=9).map(|i| AssetRef(Uuid::from_u128(i))).collect();
    assert!(TerrainViewQuery::from_mission(&mission, nine_assets, None).is_err());
    assert!(TerrainViewQuery::from_mission(&mission, vec![a1], Some(vec![])).is_err());

    // Forged corroborated = true
    let mut forged_corr = sample_raw_claim(e, c, 0x701, 0x21, now - 10);
    forged_corr.corroborated = true;
    let raw_view = RawTerrainView {
        version: VIEW_VERSION.to_string(),
        engagement_id: e,
        campaign_id: c,
        mission_revision: 1,
        registration_operation_id: mission.operation_id,
        registration_event_id: mission.event_id,
        vantage_ref: Uuid::from_u128(0x31),
        evaluated_at: now,
        selected_assets: vec![a1],
        claims: vec![forged_corr],
        omitted_by_claim_limit: 0,
    };
    assert_eq!(
        TerrainPublicView::from_raw(raw_view.clone(), &mission, &[a1]).err(),
        Some(Fail::Input("forged_terrain_summary"))
    );

    // Forged claim_id not matching TerrainIds::derive
    let mut forged_id = sample_raw_claim(e, c, 0x702, 0x21, now - 10);
    forged_id.claim_id = Uuid::from_u128(0xdead_beef);
    let mut raw_id = raw_view.clone();
    raw_id.claims = vec![forged_id];
    assert_eq!(
        TerrainPublicView::from_raw(raw_id, &mission, &[a1]).err(),
        Some(Fail::Input("forged_terrain_summary"))
    );

    // Forged valid_until exceeding 60s cap
    let mut forged_ttl = sample_raw_claim(e, c, 0x703, 0x21, now - 10);
    forged_ttl.valid_until = Some(now + 110);
    let mut raw_ttl = raw_view.clone();
    raw_ttl.claims = vec![forged_ttl];
    assert_eq!(
        TerrainPublicView::from_raw(raw_ttl, &mission, &[a1]).err(),
        Some(Fail::Input("forged_terrain_summary"))
    );

    // Forged summary injected into ContextBasis::terrain directly must also be caught by prepare
    let mut valid_view = build_view(
        &mission,
        vec![a1],
        now,
        vec![sample_raw_claim(e, c, 0x704, 0x21, now - 10)],
        0,
    );
    valid_view.claims[0].corroborated = true;
    let mut reader = FixedReader(ContextBasis {
        mission,
        capacity: default_capacity(),
        evaluated_at: now,
        terrain: TerrainContextState::Available(valid_view),
    });
    let q = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req,
        candidates: vec![Candidate {
            asset_ref: a1,
            priority: Priority::High,
        }],
    };
    assert_eq!(
        m1_context::prepare(&mut reader, &q, OperationId(Uuid::from_u128(0x904))).err(),
        Some(Fail::Input("forged_terrain_summary"))
    );
}

#[test]
fn byte_fitting_prefers_dropping_operator_only_candidate_before_sole_terrain_premise() {
    let now = 1_700_000_000;
    let (e, c) = (
        EngagementId(Uuid::from_u128(0x105)),
        CampaignId(Uuid::from_u128(0x205)),
    );
    let a1 = AssetRef(Uuid::from_u128(0x21));
    let a2 = AssetRef(Uuid::from_u128(0x22));
    let op = OperationId(Uuid::from_u128(0x905));

    let (m_full, req_full) = build_mission(e, c, now, true, 4096);
    let claim_a1 = sample_raw_claim(e, c, 0x801, 0x21, now - 10);
    let view = build_view(&m_full, vec![a1, a2], now, vec![claim_a1], 0);

    // At 4096 bytes with 2 candidates (a1 with terrain_0, a2 operator-only),
    // fitting drops optional operator-only candidate a2 before dropping terrain_0 for a1
    let q_two = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req_full,
        candidates: vec![
            Candidate {
                asset_ref: a1,
                priority: Priority::High,
            },
            Candidate {
                asset_ref: a2,
                priority: Priority::Low,
            },
        ],
    };
    let mut reader_tight = FixedReader(ContextBasis {
        mission: m_full,
        capacity: default_capacity(),
        evaluated_at: now,
        terrain: TerrainContextState::Available(view.clone()),
    });
    let tight_prep = m1_context::prepare(&mut reader_tight, &q_two, op).unwrap();
    assert_eq!(tight_prep.terrain_dependencies().len(), 1);
    assert_eq!(tight_prep.terrain_dependencies()[0].local_id, "terrain_0");
    let tight_pack: Value = serde_json::from_str(tight_prep.utf8()).unwrap();
    assert_eq!(tight_pack["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(tight_pack["omissions"]["omitted_candidates"], 1);
    assert_eq!(tight_pack["owner_views"][1]["shown_claims"], 1);
    assert_eq!(
        tight_pack["owner_views"][1]["omissions"]["omitted_by_byte_bound"],
        0
    );

    // Input position must not sacrifice the sole eligible Terrain premise.
    let mut reversed = ContextRequest {
        version: q_two.version.clone(),
        mission: q_two.mission.clone(),
        candidates: q_two.candidates.clone(),
    };
    reversed.candidates.reverse();
    let reversed_prep = m1_context::prepare(&mut reader_tight, &reversed, op).unwrap();
    assert_eq!(
        reversed_prep.terrain_dependencies(),
        tight_prep.terrain_dependencies()
    );
    assert_eq!(
        request::reference_ids(reversed_prep.utf8().as_bytes()).unwrap(),
        vec!["mission", "asset_0", "terrain_0"]
    );
    assert!(!reversed_prep.utf8().contains(&a2.0.to_string()));
    let reversed_pack: Value = serde_json::from_str(reversed_prep.utf8()).unwrap();
    assert_eq!(reversed_pack["candidates"], tight_pack["candidates"]);
    assert_eq!(reversed_pack["references"], tight_pack["references"]);
    assert_eq!(reversed_pack["owner_views"], tight_pack["owner_views"]);
    assert_eq!(reversed_pack["omissions"]["supplied_candidates"], 2);
    assert_eq!(reversed_pack["omissions"]["shown_candidates"], 1);
    assert_eq!(reversed_pack["omissions"]["omitted_candidates"], 1);
    assert_eq!(
        reversed_pack["omissions"]["selection"],
        "input_order_with_count_and_byte_bounds"
    );
    assert!(reversed_prep.utf8().len() <= 4096);

    // Consent candidate ceiling applies to the input prefix first; a Terrain-backed candidate
    // outside that authorized prefix (index 1 when candidates limit is 1) is never pulled in.
    let (m_cap1_base, req_cap1) = build_mission(e, c, now, true, 4096);
    let mut cap1_val = serde_json::to_value(&m_cap1_base).unwrap();
    cap1_val["fields"]["m1_inference_permission"]["operation_limits"]["candidates"] = json!(1);
    let m_cap1: MissionRegistered = serde_json::from_value(cap1_val).unwrap();
    let mut reader_cap1 = FixedReader(ContextBasis {
        mission: m_cap1.clone(),
        capacity: default_capacity(),
        evaluated_at: now,
        terrain: TerrainContextState::Available(build_view(
            &m_cap1,
            vec![a1, a2],
            now,
            vec![sample_raw_claim(e, c, 0x801, 0x21, now - 10)],
            0,
        )),
    });
    let q_cap1 = ContextRequest {
        version: reversed.version.clone(),
        mission: req_cap1,
        candidates: reversed.candidates.clone(),
    };
    let cap1_prep = m1_context::prepare(&mut reader_cap1, &q_cap1, op).unwrap();
    assert!(cap1_prep.terrain_dependencies().is_empty());
    let cap1_pack: Value = serde_json::from_str(cap1_prep.utf8()).unwrap();
    assert_eq!(cap1_pack["candidates"].as_array().unwrap().len(), 1);
    assert_eq!(cap1_pack["references"][1]["entity"], json!(a2.0));
    assert_eq!(
        cap1_pack["omissions"]["selection"],
        "input_order_prefix_with_count_and_byte_bounds"
    );

    // Reduce context_bytes below the single-candidate + single-claim floor:
    // byte fitting must defer with context_bytes_insufficient rather than drop the last premise.
    let tight_len = tight_prep.utf8().len();
    let (m_smaller, req_smaller) = build_mission(e, c, now, true, tight_len - 1);
    let q_smaller = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req_smaller,
        candidates: q_two.candidates.clone(),
    };
    let mut reader_smaller = FixedReader(ContextBasis {
        mission: m_smaller,
        capacity: default_capacity(),
        evaluated_at: now,
        terrain: TerrainContextState::Available(view.clone()),
    });
    assert_eq!(
        m1_context::prepare(&mut reader_smaller, &q_smaller, op).err(),
        Some(Fail::State("context_bytes_insufficient"))
    );
    let rev_len = reversed_prep.utf8().len();
    let (m_smaller_rev, req_smaller_rev) = build_mission(e, c, now, true, rev_len - 1);
    let q_smaller_rev = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req_smaller_rev,
        candidates: reversed.candidates.clone(),
    };
    let mut reader_smaller_rev = FixedReader(ContextBasis {
        mission: m_smaller_rev,
        capacity: default_capacity(),
        evaluated_at: now,
        terrain: TerrainContextState::Available(view.clone()),
    });
    assert_eq!(
        m1_context::prepare(&mut reader_smaller_rev, &q_smaller_rev, op).err(),
        Some(Fail::State("context_bytes_insufficient"))
    );

    // Without an available eligible Terrain premise for the selected prefix, tiny consent
    // preserves context_size_deferred when mandatory metadata itself cannot fit.
    let (m_tiny, req_tiny) = build_mission(e, c, now, false, 512);
    let q_tiny = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req_tiny,
        candidates: q_two.candidates,
    };
    let mut reader_tiny = FixedReader(ContextBasis {
        mission: m_tiny,
        capacity: default_capacity(),
        evaluated_at: now,
        terrain: TerrainContextState::Available(view),
    });
    assert_eq!(
        m1_context::prepare(&mut reader_tiny, &q_tiny, op).err(),
        Some(Fail::State("context_size_deferred"))
    );
}

#[test]
fn pure_dependency_recheck_cited_hook_and_binding_verification() {
    let now = 1_700_000_000;
    let (e, c) = (
        EngagementId(Uuid::from_u128(0x106)),
        CampaignId(Uuid::from_u128(0x206)),
    );
    let a1 = AssetRef(Uuid::from_u128(0x21));
    let (mission, _) = build_mission(e, c, now, true, 4096);
    let raw_claim = sample_raw_claim(e, c, 0x810, 0x21, now - 10);
    let view = build_view(&mission, vec![a1], now, vec![raw_claim], 0);
    let dep = TerrainDependency::from_claim("terrain_0".into(), &view.claims[0]).unwrap();

    // Fresh at `now` succeeds; expired at `now + 60` fails with reference_ineligible
    assert_eq!(
        verify_current_dependencies(std::slice::from_ref(&dep), &view, now),
        Ok(())
    );
    assert_eq!(
        verify_current_dependencies(std::slice::from_ref(&dep), &view, now + 60),
        Err("reference_ineligible")
    );

    // Superseded claim_revision fails
    let mut superseded_view = view.clone();
    superseded_view.claims[0].claim_revision = 2;
    assert_eq!(
        verify_current_dependencies(&[dep], &superseded_view, now),
        Err("reference_ineligible")
    );

    // Proposal admission invokes current_authority_for_references with cited union, not before_provider_release
    struct CitedTracker {
        cited: Vec<String>,
        fail_if_uncited_checked: bool,
    }
    impl ProposalBasis for CitedTracker {
        fn reference_eligible(&self, r: &str) -> bool {
            ["mission", "asset_0", "terrain_0", "terrain_1"].contains(&r)
        }
        fn manifest_eligible(&self, _: &str) -> bool {
            false
        }
        fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
            if self.fail_if_uncited_checked {
                return Err("all_premises_should_not_be_called_by_admit");
            }
            Ok(())
        }
        fn current_authority_for_references(
            &mut self,
            references: &[String],
        ) -> Result<(), &'static str> {
            self.cited = references.to_vec();
            if references.iter().any(|r| r == "terrain_1") {
                return Err("reference_ineligible");
            }
            Ok(())
        }
        fn safe_explanation(&self, _: &str) -> bool {
            true
        }
    }

    let mut tracker = CitedTracker {
        cited: Vec::new(),
        fail_if_uncited_checked: true,
    };
    let prop_json = json!({
        "version": "m1_inference_proposal_v1",
        "hypothesis": "bounded hypothesis",
        "considered_alternative": "alternative",
        "references": ["mission", "terrain_0"],
        "expected_observation": "expected",
        "falsifying_observation": "falsifying",
        "uncertainty": "bounded",
        "rationale": "concise rationale",
        "next_intent": {
            "category": "observe_more",
            "references": ["asset_0", "terrain_0"],
            "manifest_reference": null,
            "view_request": {
                "owner": "cyber_terrain",
                "selectors": ["asset_0"]
            }
        }
    });
    let adm = proposal::admit(&serde_json::to_vec(&prop_json).unwrap(), &mut tracker);
    assert!(matches!(adm, Admission::Accepted(_)));
    assert_eq!(tracker.cited, vec!["mission", "terrain_0", "asset_0"]);

    // Request byte-binding verification detects any mutation in pack, config, body, or digest
    let pack = br#"{"references":[{"id":"mission"},{"id":"terrain_0"}]}"#;
    let bound_gen = request::bind(pack, None).unwrap();
    let bound_rep = request::bind(pack, Some(&["json_structure"])).unwrap();
    let p_str = std::str::from_utf8(&bound_gen.pack).unwrap();
    let cfg_str = std::str::from_utf8(&bound_gen.configuration).unwrap();
    let body_str = std::str::from_utf8(&bound_gen.body).unwrap();
    assert!(request::verify_binding(p_str, cfg_str, body_str, &bound_gen.digest, false).is_ok());
    assert!(
        request::verify_binding(
            p_str,
            std::str::from_utf8(&bound_rep.configuration).unwrap(),
            std::str::from_utf8(&bound_rep.body).unwrap(),
            &bound_rep.digest,
            true,
        )
        .is_ok()
    );
    assert!(
        request::verify_binding(
            r#"{"references":[{"id":"mission"}]}"#,
            cfg_str,
            body_str,
            &bound_gen.digest,
            false
        )
        .is_err()
    );
    assert!(
        request::verify_binding(p_str, cfg_str, body_str, "00".repeat(32).as_str(), false).is_err()
    );
}

#[path = "support/m1_terrain_capacity.rs"]
mod m1_terrain_capacity;
