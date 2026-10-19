//! Offline diagnostic (DW-DIAGNOSE-M1-SCHEMA-DEADLINE): contract between the
//! actual emitted provider JSON Schema (`request::bind` `text.format.schema`)
//! and the local `proposal::admit` decoder. Synthetic ContextPack produced by
//! the real `m1_context::prepare`; no DB, network, credentials or live output.
//! When DW_M1_SCHEMA_DIAGNOSTIC_DIR equals the named report directory, the
//! emitted schema plus every synthetic case and observed Rust disposition are
//! exported to schema-vectors.json for the separate Python jsonschema pass.
#![recursion_limit = "256"]
use duskweave::m1_context::{self, ContextBasis, ContextReader};
use duskweave::m1_context_capacity::ContextCapacity;
use duskweave::m1_context_input::{Candidate, ContextRequest, Priority};
use duskweave::m1_inference_budget::{ReservationRequest, ReservedCapacity};
use duskweave::m1_reasoning::proposal::{self, Admission, ProposalBasis};
use duskweave::m1_reasoning::request::{self, BoundCall};
use duskweave::mission::{AssetRef, CONTRACT_KIND, MissionRegistered, OperationId, PRODUCER};
use serde_json::{Value, json};
use uuid::Uuid;

const EXPORT_ENV: &str = "DW_M1_SCHEMA_DIAGNOSTIC_DIR";
const EXPORT_DIR: &str =
    "D:/DuskWeave-build/rust-1.94.1/reports/dw-diagnose-m1-schema-deadline/cycle-1";

struct Reader(ContextBasis);
impl ContextReader for Reader {
    fn read(&mut self, _q: &ContextRequest, _op: OperationId) -> duskweave::Res<ContextBasis> {
        let b = &self.0;
        Ok(ContextBasis {
            mission: b.mission.clone(),
            capacity: ContextCapacity {
                version: b.capacity.version.clone(),
                epoch: b.capacity.epoch,
                session_claimed: b.capacity.session_claimed,
                active_holds: b.capacity.active_holds,
                charged: b.capacity.charged.clone(),
            },
            evaluated_at: b.evaluated_at,
            terrain: b.terrain.clone(),
        })
    }
}

fn raw_registration(e: Uuid, c: Uuid, now: i64) -> Value {
    json!({
        "engagement_id": e, "campaign_id": c,
        "operator_ref": Uuid::from_u128(0x11),
        "authority_ref": Uuid::from_u128(0x12), "authority_revision": 1,
        "goal_ref": Uuid::from_u128(0x13),
        "included_assets": [Uuid::from_u128(0x21)],
        "excluded_assets": [Uuid::from_u128(0x22)],
        "exercise_mode": "blind",
        "starts_at": now - 3600, "ends_at": now + 3600,
        "m1_permission": {
            "policy_version": 1, "ct_base_domain": "example.invalid",
            "provider_disclosure": "crt_sh",
            "vantage_ref": Uuid::from_u128(0x31), "resolver_ipv4": "192.0.2.53",
            "discovery_rules": [{"label_suffix": "example.invalid"}],
            "contact_rules": [{"asset_ref": Uuid::from_u128(0x21),
                "rule": {"exact": "api.example.invalid"}, "priority": 1}],
            "excluded_names": [], "approved_path": "/",
            "starts_at": now - 3500, "ends_at": now + 3500,
            "campaign_limits": {"episodes": 1, "provider_calls": 1,
                "dns_questions": 1, "dns_followups": 1,
                "tcp_connections": 1, "head_requests": 1},
            "concurrency": 1
        },
        "m1_inference_permission": {
            "version": "m1_inference_permission_v1",
            "permission_id": Uuid::from_u128(0x41),
            "engagement_id": e, "campaign_id": c,
            "operator_ref": Uuid::from_u128(0x11),
            "authority_ref": Uuid::from_u128(0x12), "authority_revision": 1,
            "mission_revision": 1, "goal_ref": Uuid::from_u128(0x13),
            "exercise_mode": "blind",
            "recipient": "https://api.deepseek.com/responses",
            "profile": "deepseek-responses-v1",
            "field_classes": ["mission_question_constraints",
                "scoped_opaque_references", "omissions_unknown", "capacity",
                "admitted_operator_premises"],
            "starts_at": now - 3500, "ends_at": now + 3500,
            "provider_terms_ref": Uuid::from_u128(0x32),
            "provider_terms_approved": true,
            "campaign_limits": {"calls": 2, "request_bytes": 32768,
                "response_bytes": 131072, "output_token_capacity": 8192,
                "wall_time_ms": 20000},
            "operation_limits": {"context_bytes": 4096, "candidates": 8,
                "request_bytes": 16384, "configuration_bytes": 16384,
                "response_bytes": 65536, "calls_per_decision": 2,
                "output_tokens_per_call": 4096,
                "decision_wall_time_ms": 20000, "connect_time_ms": 5000,
                "proposal_bytes": 8192, "repair_bytes": 2048,
                "text_field_bytes": 1024, "repair_error_codes": 8,
                "json_depth": 8, "collection_entries": 64, "concurrency": 1}
        }
    })
}

fn basis(e: Uuid, c: Uuid, now: i64, raw: &Value) -> ContextBasis {
    let op = Uuid::from_u128(0xa002);
    let mission: MissionRegistered = serde_json::from_value(json!({
        "event_id": Uuid::from_u128(0xa001),
        "operation_id": op,
        "engagement_id": e, "campaign_id": c,
        "producer": PRODUCER,
        "affected_entity": c,
        "owner_revision": 1,
        "kind": CONTRACT_KIND,
        "version": 3,
        "causation_id": op,
        "correlation_id": op,
        "occurred_at": now,
        "recorded_at": now,
        "fields": raw
    }))
    .unwrap();
    ContextBasis {
        mission,
        capacity: ContextCapacity {
            version: "m1_inference_capacity_snapshot_v1".into(),
            epoch: 1,
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
        evaluated_at: now,
        terrain: m1_context::TerrainContextState::NotLoaded,
    }
}

fn context_request(e: Uuid, raw: &Value) -> ContextRequest {
    let mission: ReservationRequest = serde_json::from_value(json!({
        "version": "m1_inference_reservation_request_v1",
        "engagement_id": e,
        "campaign_id": raw["campaign_id"],
        "operator_ref": raw["operator_ref"],
        "expected_mission_revision": 1,
        "permission_id": raw["m1_inference_permission"]["permission_id"],
        "goal_ref": raw["goal_ref"],
        "exercise_mode": "blind"
    }))
    .unwrap();
    ContextRequest {
        version: "m1_context_prepare_request_v1".into(),
        mission,
        candidates: vec![Candidate {
            asset_ref: AssetRef(Uuid::from_u128(0x21)),
            priority: Priority::High,
        }],
    }
}

/// Actual ContextPack produced by the real producer over synthetic basis.
fn pack() -> String {
    let now = 1700000000;
    let (e, c) = (Uuid::from_u128(0x101), Uuid::from_u128(0x102));
    let raw = raw_registration(e, c, now);
    let mut reader = Reader(basis(e, c, now, &raw));
    let q = context_request(e, &raw);
    m1_context::prepare(&mut reader, &q, OperationId(Uuid::from_u128(0xa003)))
        .expect("synthetic basis must prepare")
        .utf8()
        .to_owned()
}

fn emitted_schema(call: &BoundCall) -> Value {
    let body: Value = serde_json::from_slice(&call.body).unwrap();
    body["text"]["format"]["schema"].clone()
}

struct Basis<'a>(&'a [String]);
impl ProposalBasis for Basis<'_> {
    fn reference_eligible(&self, r: &str) -> bool {
        self.0.iter().any(|id| id == r)
    }
    fn manifest_eligible(&self, _: &str) -> bool {
        false
    }
    fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
    fn safe_explanation(&self, _: &str) -> bool {
        true
    }
}

fn disposition(proposal: &Value, eligible: &[String]) -> String {
    let raw = serde_json::to_vec(proposal).unwrap();
    let mut b = Basis(eligible);
    match proposal::admit(&raw, &mut b) {
        Admission::Accepted(_) => "accepted".to_owned(),
        Admission::Repairable(codes) => format!("repairable:{}", codes.join(",")),
        Admission::Rejected(code) => format!("rejected:{code}"),
    }
}

fn base_proposal() -> Value {
    json!({
        "version": "m1_inference_proposal_v1",
        "hypothesis": "bounded synthetic hypothesis",
        "considered_alternative": "synthetic alternative",
        "references": ["mission"],
        "expected_observation": "synthetic expected",
        "falsifying_observation": "synthetic falsifying",
        "uncertainty": "synthetic uncertainty",
        "rationale": "synthetic safe rationale",
        "next_intent": {"category": "observe_more", "references": ["mission"],
            "manifest_reference": null,
            "view_request": {"owner": "cyber_terrain", "selectors": ["mission"]}}
    })
}

fn with_intent(category: &str) -> Value {
    let mut p = base_proposal();
    p["next_intent"] = json!({"category": category, "references": ["mission"],
        "manifest_reference": null});
    if category == "observe_more" {
        p["next_intent"]["view_request"] =
            json!({"owner": "cyber_terrain", "selectors": ["mission"]});
    }
    p
}

struct Case {
    name: &'static str,
    proposal: Value,
    /// Expected standard-schema verdict; None means "depends on draft rules".
    expect_schema_valid: Option<bool>,
    expect_disposition_prefix: &'static str,
    /// Native restriction beyond the emitted schema, if any.
    native_extra: &'static str,
}

fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    let mut push = |name, proposal, expect, prefix, extra| {
        out.push(Case {
            name,
            proposal,
            expect_schema_valid: expect,
            expect_disposition_prefix: prefix,
            native_extra: extra,
        });
    };
    for category in [
        "validate",
        "observe_more",
        "synthesize_candidate_implementation",
        "retain",
        "defer",
        "stop",
    ] {
        let mut p = with_intent(category);
        if matches!(category, "defer" | "stop") {
            p["references"] = json!([]);
            p["next_intent"]["references"] = json!([]);
        }
        push(
            match category {
                "validate" => "valid_validate",
                "observe_more" => "valid_observe_more_view_request",
                "synthesize_candidate_implementation" => "valid_synthesize",
                "retain" => "valid_retain",
                "defer" => "valid_defer_empty_references",
                _ => "valid_stop_empty_references",
            },
            p,
            Some(true),
            "accepted",
            "none",
        );
    }
    let mut p = base_proposal();
    p["next_intent"]["view_request"] = Value::Null;
    push(
        "valid_observe_more_null_view_request",
        p,
        Some(true),
        "accepted",
        "none",
    );
    let mut p = base_proposal();
    p["next_intent"]
        .as_object_mut()
        .unwrap()
        .remove("view_request");
    push(
        "observe_more_missing_view_request_key",
        p,
        Some(false),
        "accepted",
        "schema requires key; local Option decode tolerates omission",
    );
    let mut p = base_proposal();
    p["next_intent"]
        .as_object_mut()
        .unwrap()
        .remove("manifest_reference");
    push(
        "missing_manifest_reference_key",
        p,
        Some(false),
        "accepted",
        "schema requires key; local Option decode tolerates omission",
    );
    let mut p = base_proposal();
    p.as_object_mut().unwrap().remove("rationale");
    push(
        "missing_rationale",
        p,
        Some(false),
        "repairable:proposal_shape",
        "none",
    );
    let mut p = base_proposal();
    p["hypothesis"] = json!(7);
    push(
        "wrong_type_hypothesis_number",
        p,
        Some(false),
        "repairable:proposal_shape",
        "none",
    );
    let mut p = base_proposal();
    p["surprise"] = json!(true);
    push(
        "unknown_root_field",
        p,
        Some(false),
        "repairable:proposal_shape",
        "none",
    );
    let mut p = base_proposal();
    p["next_intent"]["extra"] = json!(1);
    push(
        "unknown_nested_intent_field",
        p,
        Some(false),
        "repairable:proposal_shape",
        "none",
    );
    let mut p = base_proposal();
    p["next_intent"]["category"] = json!("improvise");
    p["next_intent"]
        .as_object_mut()
        .unwrap()
        .remove("view_request");
    push(
        "unknown_category_enum",
        p,
        Some(false),
        "repairable:proposal_shape",
        "none",
    );
    let mut p = base_proposal();
    p["next_intent"]["category"] = json!("improvise");
    push(
        "unknown_category_with_view_request",
        p,
        Some(false),
        "repairable:unexpected_view_request",
        "structural view_request check runs before enum decode",
    );
    let mut p = base_proposal();
    p["references"] = json!("mission");
    push(
        "references_scalar_not_array",
        p,
        Some(false),
        "repairable:proposal_shape",
        "none",
    );
    let mut p = base_proposal();
    p["version"] = json!("m1_inference_proposal_v0");
    push(
        "invalid_version",
        p,
        Some(false),
        "repairable:proposal_version",
        "none",
    );
    let mut p = with_intent("validate");
    p["next_intent"]["view_request"] = json!({"owner": "cyber_terrain",
        "selectors": ["mission"]});
    push(
        "view_request_on_validate",
        p,
        Some(false),
        "repairable:unexpected_view_request",
        "none",
    );
    let mut p = base_proposal();
    p["references"] = json!(["entity-uuid-not-in-enum"]);
    push(
        "ineligible_reference_id",
        p,
        Some(false),
        "rejected:reference_ineligible",
        "none",
    );
    let mut p = base_proposal();
    p["references"] = json!([]);
    push(
        "empty_references_on_observe_more",
        p,
        Some(true),
        "rejected:unsupported_premise",
        "schema has no minItems; native premise rule is stricter",
    );
    let mut p = base_proposal();
    p["next_intent"]["manifest_reference"] = json!("mission");
    push(
        "nonnull_manifest_reference",
        p,
        Some(false),
        "rejected:manifest_ineligible",
        "none",
    );
    let mut p = base_proposal();
    p["next_intent"]["view_request"]["owner"] = json!("attacker_console");
    push(
        "unknown_view_owner",
        p,
        Some(false),
        "repairable:proposal_shape",
        "none",
    );
    let mut p = base_proposal();
    p["next_intent"]["view_request"]["selectors"] = json!(["uuid-or-path"]);
    push(
        "ineligible_view_selector",
        p,
        Some(false),
        "rejected:view_selector_ineligible",
        "none",
    );
    let mut p = base_proposal();
    p["hypothesis"] = json!(" ");
    push(
        "whitespace_only_text",
        p,
        Some(true),
        "repairable:proposal_text",
        "schema minLength counts length 1; native trim rule is stricter",
    );
    let mut p = base_proposal();
    p["hypothesis"] = json!("x".repeat(1025));
    push(
        "ascii_text_over_1024",
        p,
        Some(false),
        "repairable:json_structure",
        "pre-decode per-string byte bound (1024) fires before decoder text check",
    );
    let mut p = base_proposal();
    p["hypothesis"] = json!("é".repeat(700));
    push(
        "multibyte_text_700_chars_1400_bytes",
        p,
        Some(true),
        "repairable:json_structure",
        "JSON Schema maxLength counts characters; native bound counts bytes",
    );
    let p = with_intent("defer");
    push(
        "defer_with_supported_reference",
        p,
        Some(true),
        "accepted",
        "none",
    );
    out
}

fn schema_enum_ids(schema: &Value) -> Vec<String> {
    schema["properties"]["references"]["items"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn emitted_schema_structure_and_pack_reference_enum() {
    let p = pack();
    let ids = request::reference_ids(p.as_bytes()).unwrap();
    assert_eq!(ids, vec!["mission".to_owned(), "asset_0".to_owned()]);
    let call = request::bind(p.as_bytes(), None).unwrap();
    let schema = emitted_schema(&call);
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema_enum_ids(&schema), ids);
    let required: Vec<_> = schema["required"].as_array().unwrap().clone();
    for field in [
        "version",
        "hypothesis",
        "considered_alternative",
        "expected_observation",
        "falsifying_observation",
        "uncertainty",
        "rationale",
        "references",
        "next_intent",
    ] {
        assert!(required.contains(&json!(field)), "{field} required");
    }
    let variants = schema["properties"]["next_intent"]["anyOf"]
        .as_array()
        .unwrap();
    assert_eq!(variants.len(), 6);
    let observe = variants
        .iter()
        .find(|v| v["properties"]["category"]["enum"] == json!(["observe_more"]))
        .expect("observe_more variant");
    let observed_required: Vec<_> = observe["required"].as_array().unwrap().clone();
    assert!(observed_required.contains(&json!("view_request")));
    assert!(
        observe["properties"]["view_request"]["anyOf"][0]["type"] == "null",
        "view_request is required but nullable"
    );
}

#[test]
fn generation_and_repair_binds_preserve_pack_and_binding() {
    let p = pack();
    let generation = request::bind(p.as_bytes(), None).unwrap();
    let repair = request::bind(p.as_bytes(), Some(&["proposal_shape"])).unwrap();
    assert_eq!(generation.pack, repair.pack);
    assert_eq!(generation.pack, p.as_bytes());
    request::verify_binding(
        &p,
        std::str::from_utf8(&generation.configuration).unwrap(),
        std::str::from_utf8(&generation.body).unwrap(),
        &generation.digest,
        false,
    )
    .unwrap();
    request::verify_binding(
        &p,
        std::str::from_utf8(&repair.configuration).unwrap(),
        std::str::from_utf8(&repair.body).unwrap(),
        &repair.digest,
        true,
    )
    .unwrap();
    let gen_body: Value = serde_json::from_slice(&generation.body).unwrap();
    let rep_body: Value = serde_json::from_slice(&repair.body).unwrap();
    assert_eq!(gen_body["input"], json!(p));
    let rep_input = rep_body["input"].as_array().unwrap();
    assert_eq!(rep_input.len(), 2);
    assert_eq!(rep_input[0]["content"], json!(p));
    let repair_msg: Value =
        serde_json::from_str(rep_input[1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        repair_msg,
        json!({"repair_error_codes": ["proposal_shape"]})
    );
    assert_eq!(gen_body["text"]["format"], rep_body["text"]["format"]);
    assert_eq!(gen_body["instructions"], rep_body["instructions"]);
}

#[test]
fn admission_matrix_and_optional_export() {
    let p = pack();
    let ids = request::reference_ids(p.as_bytes()).unwrap();
    let call = request::bind(p.as_bytes(), None).unwrap();
    let schema = emitted_schema(&call);
    let mut exported = Vec::new();
    for case in cases() {
        let observed = disposition(&case.proposal, &ids);
        assert!(
            observed.starts_with(case.expect_disposition_prefix),
            "{}: expected {}, observed {}",
            case.name,
            case.expect_disposition_prefix,
            observed
        );
        exported.push(json!({
            "name": case.name,
            "proposal": case.proposal,
            "expect_schema_valid": case.expect_schema_valid,
            "rust_disposition": observed,
            "native_extra": case.native_extra,
        }));
    }
    let Some(dir) = std::env::var_os(EXPORT_ENV) else {
        return;
    };
    let dir = dir.to_string_lossy().replace('\\', "/");
    if dir != EXPORT_DIR {
        return;
    }
    let out = json!({
        "kind": "m1_schema_diagnostic_v1",
        "schema": schema,
        "pack_reference_ids": ids,
        "pack_bytes": p.len(),
        "cases": exported,
    });
    let path = std::path::Path::new(&dir).join("schema-vectors.json");
    let mut target = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .expect("schema-vectors.json must not collide");
    use std::io::Write;
    target
        .write_all(serde_json::to_string_pretty(&out).unwrap().as_bytes())
        .unwrap();
    target.flush().unwrap();
}
