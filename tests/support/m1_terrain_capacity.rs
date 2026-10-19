//! Focused capacity, numeric-width boundary, request-binding, and minimum-premise floor matrix.
use super::*;

const REPAIR_CODES: &[&str] = &[
    "json_size",
    "json_structure",
    "json_trailing",
    "proposal_shape",
    "proposal_version",
    "unexpected_view_request",
    "proposal_text",
];

fn assert_bound_within_limits(utf8: &str) -> (usize, usize, usize, usize, usize) {
    let p_bytes = utf8.as_bytes();
    assert!(p_bytes.len() <= 4096, "P bytes {} > 4096", p_bytes.len());
    let first = request::bind(p_bytes, None).expect("generation bind must succeed");
    let rep = request::bind(p_bytes, Some(REPAIR_CODES)).expect("repair bind must succeed");
    assert!(first.configuration.len() <= 16384 && first.body.len() <= 16384);
    assert!(rep.configuration.len() <= 16384 && rep.body.len() <= 16384);
    let gen_cfg = std::str::from_utf8(&first.configuration).unwrap();
    let gen_body = std::str::from_utf8(&first.body).unwrap();
    let rep_cfg = std::str::from_utf8(&rep.configuration).unwrap();
    let rep_body = std::str::from_utf8(&rep.body).unwrap();
    request::verify_binding(utf8, gen_cfg, gen_body, &first.digest, false).unwrap();
    request::verify_binding(utf8, rep_cfg, rep_body, &rep.digest, true).unwrap();
    (
        p_bytes.len(),
        first.configuration.len(),
        first.body.len(),
        rep.configuration.len(),
        rep.body.len(),
    )
}

fn prepare_with(
    mission: MissionRegistered,
    evaluated_at: i64,
    view: TerrainPublicView,
    q: &ContextRequest,
    op: OperationId,
) -> duskweave::Res<duskweave::m1_context::PreparedContext> {
    prepare_with_epoch(mission, evaluated_at, 3, view, q, op)
}

fn prepare_with_epoch(
    mission: MissionRegistered,
    evaluated_at: i64,
    epoch: i64,
    view: TerrainPublicView,
    q: &ContextRequest,
    op: OperationId,
) -> duskweave::Res<duskweave::m1_context::PreparedContext> {
    let mut cap = default_capacity();
    cap.epoch = epoch;
    let mut reader = FixedReader(ContextBasis {
        mission,
        capacity: cap,
        evaluated_at,
        terrain: TerrainContextState::Available(view),
    });
    m1_context::prepare(&mut reader, q, op)
}

fn one_cand_req(req: ReservationRequest, asset: AssetRef, priority: Priority) -> ContextRequest {
    ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req,
        candidates: vec![Candidate {
            asset_ref: asset,
            priority,
        }],
    }
}

#[test]
fn historical_shape_two_limitations_retains_premise_and_deduplicates_losslessly() {
    let (perceived_at, evaluated_at) = (1_792_334_997, 1_792_335_054);
    let (e, c) = (
        EngagementId(Uuid::from_u128(0x9df4_d8b1)),
        CampaignId(Uuid::from_u128(0xeade_bdd0)),
    );
    let (a1, op) = (
        AssetRef(Uuid::from_u128(0x21)),
        OperationId(Uuid::from_u128(0x37eb_c0bf)),
    );
    let (mut mission, req) = build_mission(e, c, evaluated_at, true, 4096);
    let mut m_val = serde_json::to_value(&mission).unwrap();
    m_val["occurred_at"] = json!(1_792_335_053);
    m_val["fields"]["m1_inference_permission"]["campaign_limits"] = json!({
        "calls": 2, "request_bytes": 32768, "response_bytes": 131072,
        "output_token_capacity": 8192, "wall_time_ms": 20000
    });
    mission = serde_json::from_value(m_val).unwrap();

    let mut claim = sample_raw_claim(e, c, 0xf767_8fa6, 0x21, perceived_at);
    claim.lowest_ttl_seconds = Some(1799);
    claim.valid_until = Some(perceived_at + 60);
    claim.uncertainty_seconds = 1;
    claim.limitations = vec![
        DnsLimitation::CacheRemainingTtl,
        DnsLimitation::SingleVantageOnly,
    ];
    let view = build_view(&mission, vec![a1], evaluated_at, vec![claim], 0);
    let q = one_cand_req(req, a1, Priority::High);
    let prep = prepare_with_epoch(mission, evaluated_at, 2, view, &q, op).unwrap();
    assert_eq!(prep.terrain_dependencies().len(), 1);
    assert_eq!(prep.terrain_dependencies()[0].local_id, "terrain_0");
    let (p_len, gen_cfg, gen_body, rep_cfg, rep_body) = assert_bound_within_limits(prep.utf8());
    assert_eq!(
        (p_len, gen_cfg, gen_body, rep_cfg, rep_body),
        (3899, 5462, 8891, 5462, 9111)
    );

    let pack: Value = serde_json::from_str(prep.utf8()).unwrap();
    assert_eq!(
        pack["pending_unknown"],
        json!(["other_owner_views_not_loaded_by_profile"])
    );
    let views = pack["owner_views"].as_array().unwrap();
    assert_eq!(
        (
            views.len(),
            views[1]["shown_claims"].as_i64(),
            views[1]["omissions"]["omitted_by_byte_bound"].as_i64()
        ),
        (6, Some(1), Some(0))
    );
    for (idx, expected) in [
        "foothold_graph",
        "attack_path_view",
        "objective_state",
        "campaign_trajectory",
    ]
    .iter()
    .enumerate()
    {
        let v = &views[idx + 2];
        assert_eq!(
            (v["owner"].as_str(), v["availability"].as_str()),
            (Some(*expected), Some("unavailable"))
        );
        assert!(v["revision"].is_null() && v["frontier"].is_null() && v.get("reason").is_none());
        assert_eq!(v["evaluated_at"], evaluated_at);
    }
    assert_eq!(pack["references"][1]["source"], "caller_declared_premise");
    assert_eq!(pack["references"][1]["status"], "premise");
    assert_eq!(pack["references"][1]["freshness"], "unknown");
    assert_eq!(pack["references"][1]["completeness"], "declared_only");
    assert_eq!(pack["candidates"][0]["status"], "operator_premise");
    assert_eq!(
        pack["candidates"][0]["terrain_references"],
        json!(["terrain_0"])
    );
    assert_eq!(pack["remaining_capacity"]["reservation_granted"], false);
    assert_eq!(
        pack["remaining_capacity"]["shared_call_clock"],
        "not_started"
    );
}

#[test]
fn all_routing_eligible_limitations_and_numeric_width_boundaries_fit_full_profile() {
    let (e, c) = (
        EngagementId(Uuid::from_u128(0x3101)),
        CampaignId(Uuid::from_u128(0x3201)),
    );
    let (a1, op, now) = (
        AssetRef(Uuid::from_u128(0x21)),
        OperationId(Uuid::from_u128(0x3901)),
        1_792_335_054,
    );
    let (mission, req) = build_mission(e, c, now, true, 4096);

    let mut bad_alias = sample_raw_claim(e, c, 0x3500, 0x21, now - 10);
    bad_alias.limitations = vec![DnsLimitation::PartialAliasChain];
    let mut bad_dup = sample_raw_claim(e, c, 0x3501, 0x21, now - 10);
    bad_dup.limitations = vec![
        DnsLimitation::SingleVantageOnly,
        DnsLimitation::SingleVantageOnly,
    ];
    let mut bad_hold = sample_raw_claim(e, c, 0x3502, 0x21, now - 10);
    bad_hold.hold_frontier = Some(now - 5);
    for bad in [bad_alias, bad_dup, bad_hold] {
        let raw = RawTerrainView {
            version: VIEW_VERSION.to_string(),
            engagement_id: e,
            campaign_id: c,
            mission_revision: 1,
            registration_operation_id: mission.operation_id,
            registration_event_id: mission.event_id,
            vantage_ref: Uuid::from_u128(0x31),
            evaluated_at: now,
            selected_assets: vec![a1],
            claims: vec![bad],
            omitted_by_claim_limit: 0,
        };
        assert_eq!(
            TerrainPublicView::from_raw(raw, &mission, &[a1]).err(),
            Some(Fail::Input("forged_terrain_summary"))
        );
    }

    let eligible_variants = [
        DnsLimitation::CacheRemainingTtl,
        DnsLimitation::SingleVantageOnly,
        DnsLimitation::ResolverRecursionVariance,
    ];
    let q = one_cand_req(req, a1, Priority::High);
    let mut max_subset_p = 0;
    for mask in 0u8..8 {
        let lims: Vec<DnsLimitation> = eligible_variants
            .iter()
            .enumerate()
            .filter_map(|(bit, v)| ((mask & (1 << bit)) != 0).then_some(*v))
            .collect();
        let mut claim = sample_raw_claim(e, c, 0x3600 + u128::from(mask), 0x21, now - 10);
        claim.limitations = lims;
        let view = build_view(&mission, vec![a1], now, vec![claim], 0);
        let prep = prepare_with(mission.clone(), now, view, &q, op).unwrap();
        assert_eq!(prep.terrain_dependencies().len(), 1);
        let (p_len, _, _, _, _) = assert_bound_within_limits(prep.utf8());
        max_subset_p = max_subset_p.max(p_len);
    }
    assert_eq!(max_subset_p, 3932);

    let pg_max_now: i64 = 9_224_318_015_999;
    let all_assets: Vec<Uuid> = (1..=64).map(|i| Uuid::from_u128(0x20 + i)).collect();
    let (mut m_wide, req_wide) = build_mission(e, c, pg_max_now, true, 4096);
    let mut mw_val = serde_json::to_value(&m_wide).unwrap();
    mw_val["occurred_at"] = json!(pg_max_now - 10);
    mw_val["recorded_at"] = json!(pg_max_now - 10);
    mw_val["fields"]["included_assets"] = json!(all_assets);
    mw_val["fields"]["excluded_assets"] = json!([]);
    mw_val["fields"]["starts_at"] = json!(0);
    mw_val["fields"]["ends_at"] = json!(i64::MAX);
    mw_val["fields"]["m1_permission"]["starts_at"] = json!(0);
    mw_val["fields"]["m1_permission"]["ends_at"] = json!(i64::MAX);
    mw_val["fields"]["m1_inference_permission"]["starts_at"] = json!(0);
    mw_val["fields"]["m1_inference_permission"]["ends_at"] = json!(i64::MAX);
    mw_val["fields"]["m1_inference_permission"]["operation_limits"]["candidates"] = json!(1);
    mw_val["fields"]["m1_inference_permission"]["campaign_limits"] = json!({
        "calls": i64::MAX, "request_bytes": i64::MAX, "response_bytes": i64::MAX,
        "output_token_capacity": i64::MAX, "wall_time_ms": i64::MAX
    });
    m_wide = serde_json::from_value(mw_val).unwrap();

    let mut claim_intake = sample_raw_claim(e, c, 0x3701, 0x21, pg_max_now - 60);
    claim_intake.claim_revision = 10_001;
    claim_intake.previous_claim_revision = 10_000;
    claim_intake.temporal_frontier = pg_max_now - 1;
    claim_intake.valid_until = Some(pg_max_now);
    claim_intake.uncertainty_seconds = 300;
    claim_intake.lowest_ttl_seconds = Some(86_400);
    claim_intake.limitations = eligible_variants.to_vec();
    let view_intake = build_view(
        &m_wide,
        vec![a1],
        pg_max_now - 1,
        vec![claim_intake.clone()],
        0,
    );
    let q_wide = ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission: req_wide,
        candidates: all_assets
            .iter()
            .map(|u| Candidate {
                asset_ref: AssetRef(*u),
                priority: Priority::Medium,
            })
            .collect(),
    };
    let prep_intake = prepare_with_epoch(
        m_wide.clone(),
        pg_max_now - 1,
        i64::MAX,
        view_intake,
        &q_wide,
        op,
    )
    .unwrap();
    assert_eq!(prep_intake.terrain_dependencies().len(), 1);
    let (intake_p, intake_gcfg, intake_gbody, intake_rcfg, intake_rbody) =
        assert_bound_within_limits(prep_intake.utf8());
    assert_eq!(
        (
            intake_p,
            intake_gcfg,
            intake_gbody,
            intake_rcfg,
            intake_rbody
        ),
        (4075, 5462, 9069, 5462, 9289)
    );

    let mut claim_u32 = claim_intake;
    claim_u32.claim_revision = u32::MAX;
    claim_u32.previous_claim_revision = u32::MAX - 1;
    let view_u32 = build_view(&m_wide, vec![a1], pg_max_now - 1, vec![claim_u32], u32::MAX);
    let prep_u32 =
        prepare_with_epoch(m_wide, pg_max_now - 1, i64::MAX, view_u32, &q_wide, op).unwrap();
    assert_eq!(prep_u32.terrain_dependencies().len(), 1);
    let (u32_p, u32_gcfg, u32_gbody, u32_rcfg, u32_rbody) =
        assert_bound_within_limits(prep_u32.utf8());
    assert_eq!(
        (u32_p, u32_gcfg, u32_gbody, u32_rcfg, u32_rbody),
        (4094, 5462, 9088, 5462, 9308)
    );
}

#[test]
fn minimum_retained_premise_floor_and_multi_candidate_orders_never_drop_last_premise() {
    let now = 1_792_335_054;
    let (e, c) = (
        EngagementId(Uuid::from_u128(0x4101)),
        CampaignId(Uuid::from_u128(0x4201)),
    );
    let (a1, a2, op) = (
        AssetRef(Uuid::from_u128(0x21)),
        AssetRef(Uuid::from_u128(0x22)),
        OperationId(Uuid::from_u128(0x4901)),
    );

    let (m_one, req_one) = build_mission(e, c, now, true, 4096);
    let claim_a1 = sample_raw_claim(e, c, 0x4501, 0x21, now - 10);
    let view_one = build_view(&m_one, vec![a1], now, vec![claim_a1.clone()], 0);
    let q_one = one_cand_req(req_one, a1, Priority::High);
    let prep_one = prepare_with(m_one, now, view_one.clone(), &q_one, op).unwrap();
    let min_one_len = prep_one.utf8().len();
    assert_eq!(prep_one.terrain_dependencies().len(), 1);

    let (m_exact, req_exact) = build_mission(e, c, now, true, min_one_len);
    let prep_exact = prepare_with(
        m_exact,
        now,
        view_one.clone(),
        &one_cand_req(req_exact, a1, Priority::High),
        op,
    )
    .expect("exact minimal 1-premise length must fit");
    assert_eq!(
        (
            prep_exact.utf8().len(),
            prep_exact.terrain_dependencies().len()
        ),
        (min_one_len, 1)
    );

    let (m_below, req_below) = build_mission(e, c, now, true, min_one_len - 1);
    assert_eq!(
        prepare_with(
            m_below,
            now,
            view_one,
            &one_cand_req(req_below, a1, Priority::High),
            op,
        )
        .err(),
        Some(Fail::State("context_bytes_insufficient"))
    );

    let (m_two, req_two) = build_mission(e, c, now, true, 4096);
    let view_two = build_view(&m_two, vec![a1, a2], now, vec![claim_a1.clone()], 0);
    let cands_forward = vec![
        Candidate {
            asset_ref: a1,
            priority: Priority::High,
        },
        Candidate {
            asset_ref: a2,
            priority: Priority::Low,
        },
    ];
    let mut cands_reverse = cands_forward.clone();
    cands_reverse.reverse();

    for cands in [cands_forward.clone(), cands_reverse] {
        let q_cands = ContextRequest {
            version: "m1_context_prepare_request_v1".into(),
            mission: req_two.clone(),
            candidates: cands.clone(),
        };
        let dropped = prepare_with(m_two.clone(), now, view_two.clone(), &q_cands, op).unwrap();
        assert_eq!(dropped.terrain_dependencies().len(), 1);
        let dropped_json: Value = serde_json::from_str(dropped.utf8()).unwrap();
        assert_eq!(dropped_json["omissions"]["supplied_candidates"], 2);
        assert_eq!(dropped_json["omissions"]["shown_candidates"], 1);
        assert_eq!(dropped_json["omissions"]["omitted_candidates"], 1);
        assert_eq!(dropped_json["owner_views"][1]["shown_claims"], 1);
        assert_eq!(
            dropped_json["owner_views"][1]["omissions"]["omitted_by_byte_bound"],
            0
        );
        let dropped_len = dropped.utf8().len();
        let (m_exact_two, req_exact_two) = build_mission(e, c, now, true, dropped_len);
        let fit_exact = prepare_with(
            m_exact_two,
            now,
            view_two.clone(),
            &ContextRequest {
                version: q_cands.version.clone(),
                mission: req_exact_two,
                candidates: cands.clone(),
            },
            op,
        )
        .unwrap();
        assert_eq!(
            (
                fit_exact.utf8().len(),
                fit_exact.terrain_dependencies().len()
            ),
            (dropped_len, 1)
        );

        let (m_floor, req_floor) = build_mission(e, c, now, true, dropped_len - 1);
        assert_eq!(
            prepare_with(
                m_floor,
                now,
                view_two.clone(),
                &ContextRequest {
                    version: q_cands.version,
                    mission: req_floor,
                    candidates: cands
                },
                op,
            )
            .err(),
            Some(Fail::State("context_bytes_insufficient"))
        );
    }

    let claim_a2 = sample_raw_claim(e, c, 0x4502, 0x22, now - 10);
    let view_both = build_view(&m_two, vec![a1, a2], now, vec![claim_a1, claim_a2], 0);
    let kept_one = prepare_with(
        m_two,
        now,
        view_both.clone(),
        &ContextRequest {
            version: "m1_context_prepare_request_v1".into(),
            mission: req_two,
            candidates: cands_forward.clone(),
        },
        op,
    )
    .unwrap();
    assert_eq!(kept_one.terrain_dependencies().len(), 1);
    let kept_one_json: Value = serde_json::from_str(kept_one.utf8()).unwrap();
    assert_eq!(kept_one_json["owner_views"][1]["shown_claims"], 1);
    assert_eq!(
        kept_one_json["owner_views"][1]["omissions"]["omitted_by_byte_bound"],
        1
    );
    let kept_one_len = kept_one.utf8().len();
    let (m_both_exact, req_both_exact) = build_mission(e, c, now, true, kept_one_len);
    let fit_both_exact = prepare_with(
        m_both_exact,
        now,
        view_both.clone(),
        &ContextRequest {
            version: "m1_context_prepare_request_v1".into(),
            mission: req_both_exact,
            candidates: cands_forward.clone(),
        },
        op,
    )
    .unwrap();
    assert_eq!(fit_both_exact.utf8().len(), kept_one_len);

    let (m_both_below, req_both_below) = build_mission(e, c, now, true, kept_one_len - 1);
    assert_eq!(
        prepare_with(
            m_both_below,
            now,
            view_both,
            &ContextRequest {
                version: "m1_context_prepare_request_v1".into(),
                mission: req_both_below,
                candidates: cands_forward,
            },
            op,
        )
        .err(),
        Some(Fail::State("context_bytes_insufficient"))
    );
}

#[test]
fn historical_pre_deduplication_bindings_still_verify_without_reserialization() {
    let legacy_pack = json!({
        "version": "m1_context_pack_v1",
        "references": [{"id": "mission", "owner": "mission"}, {"id": "asset_0", "owner": "operator"}],
        "owner_views": [
            {"owner": "mission", "availability": "available"},
            {"owner": "cyber_terrain", "availability": "unavailable", "reason": "disclosure_not_permitted"},
            {"owner": "foothold_graph", "availability": "unavailable", "reason": "not_loaded_by_this_profile"},
            {"owner": "attack_path_view", "availability": "unavailable", "reason": "not_loaded_by_this_profile"},
            {"owner": "objective_state", "availability": "unavailable", "reason": "not_loaded_by_this_profile"},
            {"owner": "campaign_trajectory", "availability": "unavailable", "reason": "not_loaded_by_this_profile"}
        ],
        "pending_unknown": [
            "environmental_views_unavailable",
            "operator_premises_unverified",
            "send_admission_pending"
        ]
    })
    .to_string();
    assert_bound_within_limits(&legacy_pack);
}
