#[path = "request_autonomous.rs"]
mod autonomous;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

pub struct BoundCall {
    pub pack: Vec<u8>,
    pub configuration: Vec<u8>,
    pub body: Vec<u8>,
    pub digest: String,
}

const INSTRUCTIONS: &str = "DuskWeave initial inference v3: reason over labeled evidence and owner-qualified Terrain DNS summaries toward the authorized mission objective. APT41, Lazarus and Volt Typhoon are behavioral lenses for hypothesis testing, alternatives, adaptation, continuity and restrained footprint, not playbooks or permission. Missing views stay unavailable; stale, withheld, held or conflicted Terrain summaries are not active routing premises. Propose one bounded hypothesis, alternative, expected and falsifying observation and next intent. Reference fields and view selectors must use exact references[].id strings from this immutable pack only, preserving their bytes. Entity UUIDs, lineage/publication/permission/goal IDs, owner names, field names and JSON paths are not reference IDs. Empty references may support defer/stop when appropriate; other categories still require supported premises. The manifest catalog is unavailable in this lane: manifest_reference must be null. Do not invent facts, access, manifests, endpoints or authority. Defer or request a bounded safe view when support is missing. Return concise decision explanation, never private reasoning, secrets or client content. Output grants no dispatch.";

fn object(properties: Map<String, Value>) -> Value {
    let required: Vec<_> = properties.keys().cloned().collect();
    json!({"type":"object","properties":properties,"required":required,
        "additionalProperties":false})
}

pub fn reference_ids(pack: &[u8]) -> Result<Vec<String>, &'static str> {
    let value =
        crate::m1_reasoning::bounded_json::parse(pack, 8192, 1024).map_err(|_| "request_encode")?;
    if pack.len() > pack_limit(&value) {
        return Err("request_encode");
    }
    let references = value
        .get("references")
        .and_then(Value::as_array)
        .ok_or("request_encode")?;
    let mut ids = Vec::with_capacity(references.len());
    for entry in references {
        let id = reference_id(entry)?;
        if ids.iter().any(|old| old == id) {
            return Err("request_encode");
        }
        ids.push(id.to_owned());
    }
    Ok(ids)
}
fn reference_id(entry: &Value) -> Result<&str, &'static str> {
    entry
        .as_object()
        .and_then(|v| v.get("id"))
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty() && id.len() <= 1024)
        .ok_or("request_encode")
}

fn references(ids: &[String]) -> Value {
    let items = if ids.is_empty() {
        json!({"type":"string"})
    } else {
        json!({"type":"string","enum":ids})
    };
    json!({"type":"array","items":items,"maxItems":if ids.is_empty() {0} else {64}})
}

fn intent(category: &str, ids: &[String]) -> Value {
    let mut fields = Map::from_iter([
        (
            "category".into(),
            json!({"type":"string","enum":[category]}),
        ),
        ("references".into(), references(ids)),
        ("manifest_reference".into(), json!({"type":"null"})),
    ]);
    if category == "observe_more" {
        let view = object(Map::from_iter([
            (
                "owner".into(),
                json!({"type":"string","enum":["cyber_terrain",
                "foothold_graph","attack_path_view","objective_state","campaign_trajectory"]}),
            ),
            ("selectors".into(), references(ids)),
        ]));
        fields.insert(
            "view_request".into(),
            json!({"anyOf":[{"type":"null"},view]}),
        );
    }
    object(fields)
}

fn schema(ids: &[String]) -> Value {
    let mut fields = Map::new();
    fields.insert(
        "version".into(),
        json!({"type":"string","enum":["m1_inference_proposal_v1"]}),
    );
    for name in [
        "hypothesis",
        "considered_alternative",
        "expected_observation",
        "falsifying_observation",
        "uncertainty",
        "rationale",
    ] {
        fields.insert(
            name.into(),
            json!({"type":"string","minLength":1,"maxLength":1024}),
        );
    }
    fields.insert("references".into(), references(ids));
    let variants: Vec<_> = [
        "validate",
        "observe_more",
        "synthesize_candidate_implementation",
        "retain",
        "defer",
        "stop",
    ]
    .into_iter()
    .map(|category| intent(category, ids))
    .collect();
    fields.insert("next_intent".into(), json!({"anyOf":variants}));
    object(fields)
}

fn bytes(value: &Value, cap: usize) -> Result<Vec<u8>, &'static str> {
    let raw = serde_json::to_vec(value).map_err(|_| "request_encode")?;
    if raw.len() > cap {
        return Err("request_size");
    }
    Ok(raw)
}

pub fn bind(pack: &[u8], repair_codes: Option<&[&str]>) -> Result<BoundCall, &'static str> {
    let ids = reference_ids(pack)?;
    let parsed: Value = serde_json::from_slice(pack).map_err(|_| "request_encode")?;
    if matches!(
        parsed["version"].as_str(),
        Some("m1_context_pack_autonomous_v1" | "m1_context_pack_autonomous_v2")
    ) {
        return autonomous::bind(pack, &parsed, &ids, repair_codes);
    }
    bind_legacy(pack, &ids, repair_codes)
}

fn bind_legacy(
    pack: &[u8],
    ids: &[String],
    repair_codes: Option<&[&str]>,
) -> Result<BoundCall, &'static str> {
    let p = std::str::from_utf8(pack).map_err(|_| "request_encode")?;
    let schema = schema(ids);
    let input = input(p, repair_codes)?;
    let settings = json!({"model":"deepseek-flash","stream":false,
        "reasoning":{"effort":"high"},"tools":[],"tool_choice":"none",
        "max_output_tokens":4096});
    let mut request = settings.clone();
    request["instructions"] = json!(INSTRUCTIONS);
    request["input"] = input;
    request["text"] = json!({"format":{"type":"json_schema",
        "name":"m1_inference_proposal_v1","schema":schema}});
    let configuration = bytes(
        &json!({"recipient":"https://api.deepseek.com/responses",
        "method":"POST","content_type":"application/json","profile":"deepseek-responses-envelope32-v1",
        "schema_version":"m1_inference_proposal_v1","schema":schema,"instructions":INSTRUCTIONS,
        "template_version":"m1-initial-inference-v3","adapter_release":"native-libcurl-v1",
        "requested_settings":settings,"omissions":["temperature","top_p"],
        "transport":{"redirects":false,"retries":false,"proxy":false,"compression":false,
        "normal_certificate_hostname_validation":true,
        "http_response_policy":"bounded-http1.1-interim-then-final-200-no-upgrade-identity-only",
        "json_refusal_policy":"bounded-json-refusal-v2-envelope32-proposal8"},
        "limits":{"request_bytes":16384,"response_bytes":65536,"shared_wall_time_ms":20000,
        "connect_time_ms":5000,"configuration_bytes":16384,"context_bytes":4096,
        "proposal_bytes":8192,"repair_bytes":2048,"text_field_bytes":1024,
        "json_depth":crate::m1_reasoning::bounded_json::DEFAULT_JSON_DEPTH,
        "response_envelope_json_depth":crate::m1_reasoning::bounded_json::RESPONSE_ENVELOPE_JSON_DEPTH,"collection_entries":64,"calls_per_decision":2,"concurrency":1}}),
        16384,
    )?;
    let body = bytes(&request, 16384)?;
    let digest = compute_binding(pack, &configuration, &body);
    Ok(BoundCall {
        pack: pack.to_vec(),
        configuration,
        body,
        digest,
    })
}

pub fn compute_binding(pack: &[u8], configuration: &[u8], body: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b"dw-inference-binding-v1\0");
    for raw in [pack, configuration, body] {
        h.update((raw.len() as u64).to_be_bytes());
        h.update(raw);
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn verify_binding(
    pack: &str,
    configuration: &str,
    body: &str,
    expected_binding: &str,
    expected_repair: bool,
) -> Result<(), &'static str> {
    if configuration.len() > 16384 || body.len() > 16384 {
        return Err("inference_decision_integrity");
    }
    reference_ids(pack.as_bytes()).map_err(|_| "inference_decision_integrity")?;
    if compute_binding(pack.as_bytes(), configuration.as_bytes(), body.as_bytes())
        != expected_binding
    {
        return Err("inference_decision_integrity");
    }
    let parsed: Value = serde_json::from_str(pack).map_err(|_| "inference_decision_integrity")?;
    if parsed["version"] == "m1_context_pack_autonomous_v2" {
        return verify_successor(pack, configuration, body, expected_repair);
    }
    verify_body_pack(pack, body, expected_repair)
}

fn verify_successor(
    pack: &str,
    configuration: &str,
    body: &str,
    repair: bool,
) -> Result<(), &'static str> {
    let c: Value =
        serde_json::from_str(configuration).map_err(|_| "inference_decision_integrity")?;
    let b: Value = serde_json::from_str(body).map_err(|_| "inference_decision_integrity")?;
    let codes = successor_repair_codes(&c, &b, repair)?;
    let refs: Vec<_> = codes.iter().map(String::as_str).collect();
    let expected = bind(pack.as_bytes(), repair.then_some(refs.as_slice()))?;
    if expected.configuration != configuration.as_bytes() || expected.body != body.as_bytes() {
        return Err("inference_decision_integrity");
    }
    Ok(())
}
fn successor_repair_codes(c: &Value, b: &Value, repair: bool) -> Result<Vec<String>, &'static str> {
    if !repair {
        return Ok(Vec::new());
    }
    let material = if c["protocol"] == "chat-completions-v1" {
        &b["messages"][2]["content"]
    } else {
        &b["input"][1]["content"]
    };
    let v: Value = serde_json::from_str(material.as_str().ok_or("inference_decision_integrity")?)
        .map_err(|_| "inference_decision_integrity")?;
    v["repair_error_codes"]
        .as_array()
        .ok_or("inference_decision_integrity")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or("inference_decision_integrity")
        })
        .collect::<Result<Vec<_>, _>>()
}

fn verify_body_pack(pack: &str, body: &str, expected_repair: bool) -> Result<(), &'static str> {
    let v: Value = serde_json::from_str(body).map_err(|_| "inference_decision_integrity")?;
    let ok = if expected_repair {
        v.get("input").and_then(Value::as_array).is_some_and(|arr| {
            arr.len() == 2 && arr[0].get("content").and_then(Value::as_str) == Some(pack)
        })
    } else {
        v.get("input").and_then(Value::as_str) == Some(pack)
    };
    ok.then_some(()).ok_or("inference_decision_integrity")
}

pub(super) fn input(pack: &str, codes: Option<&[&str]>) -> Result<Value, &'static str> {
    const REPAIR_CODES: &[&str] = &[
        "json_size",
        "json_structure",
        "json_trailing",
        "proposal_shape",
        "proposal_version",
        "unexpected_view_request",
        "proposal_text",
    ];
    let Some(codes) = codes else {
        return Ok(json!(pack));
    };
    if codes.iter().any(|code| !REPAIR_CODES.contains(code)) {
        return Err("repair_code_ineligible");
    }
    if codes.len() > 8 {
        return Err("repair_code_limit");
    }
    let repair =
        serde_json::to_string(&json!({"repair_error_codes":codes})).map_err(|_| "repair_encode")?;
    Ok(json!([
        {"role":"user","content":pack},
        {"role":"user","content":repair}
    ]))
}

fn pack_limit(value: &Value) -> usize {
    if matches!(
        value["version"].as_str(),
        Some("m1_context_pack_autonomous_v1" | "m1_context_pack_autonomous_v2")
    ) {
        8192
    } else {
        4096
    }
}
