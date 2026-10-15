use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

pub struct BoundCall {
    pub pack: Vec<u8>,
    pub configuration: Vec<u8>,
    pub body: Vec<u8>,
    pub digest: String,
}

const INSTRUCTIONS: &str = "DuskWeave initial inference v2: reason over labeled evidence toward the authorized mission objective. APT41, Lazarus and Volt Typhoon are behavioral lenses for hypothesis testing, alternatives, adaptation, continuity and restrained footprint, not playbooks or permission. Missing views stay unavailable. Propose one bounded hypothesis, alternative, expected and falsifying observation and next intent. Reference fields and view selectors must use exact references[].id strings from this immutable pack only, preserving their bytes. Entity UUIDs, lineage/publication/permission/goal IDs, owner names, field names and JSON paths are not reference IDs. Empty references may support defer/stop when appropriate; other categories still require supported premises. The manifest catalog is unavailable in this lane: manifest_reference must be null. Do not invent facts, access, manifests, endpoints or authority. Defer or request a bounded safe view when support is missing. Return concise decision explanation, never private reasoning, secrets or client content. Output grants no dispatch.";

fn object(properties: Map<String, Value>) -> Value {
    let required: Vec<_> = properties.keys().cloned().collect();
    json!({"type":"object","properties":properties,"required":required,
        "additionalProperties":false})
}

pub fn reference_ids(pack: &[u8]) -> Result<Vec<String>, &'static str> {
    let value =
        crate::m1_reasoning::bounded_json::parse(pack, 4096, 1024).map_err(|_| "request_encode")?;
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
    let p = std::str::from_utf8(pack).map_err(|_| "request_encode")?;
    let schema = schema(&ids);
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
        "template_version":"m1-initial-inference-v2","adapter_release":"native-libcurl-v1",
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
    let mut h = Sha256::new();
    h.update(b"dw-inference-binding-v1\0");
    for raw in [pack, configuration.as_slice(), body.as_slice()] {
        h.update((raw.len() as u64).to_be_bytes());
        h.update(raw);
    }
    let digest = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    Ok(BoundCall {
        pack: pack.to_vec(),
        configuration,
        body,
        digest,
    })
}

fn input(pack: &str, codes: Option<&[&str]>) -> Result<Value, &'static str> {
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
