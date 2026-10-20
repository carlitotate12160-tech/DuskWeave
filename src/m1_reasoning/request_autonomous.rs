//! Autonomous request template; legacy binding remains byte-for-byte unchanged.
use super::{BoundCall, bytes, compute_binding, input, schema};
use serde_json::{Value, json};

fn template() -> Value {
    json!({
        "adapter_release": "autonomous-libcurl-v1",
        "content_type": "application/json",
        "execution_mode": "live",
        "instructions": "DuskWeave autonomous inference v1: propose one objective-led hypothesis, alternative, expected and falsifying observation and next intent over this immutable pack. APT41, Lazarus and Volt Typhoon are behavioral lenses, not playbooks or permission. Operator seeds are premises, observations are not facts, and historical decisions do not grant current authority. Use exact references[].id and qualified manifests[].reference values only. observe_more with a DNS manifest proposes one observation for one seed; observe_more with view_request requests only an owner view. Do not combine them. Do not invent endpoints, references, availability, access or scope. Missing, stale, conflicted and fixture sources retain their limits. Explain uncertainty and any retention/revision from predecessor feedback. Retain, defer and stop are valid choices. Synthesis/unsupported effects are explanations of a capability gap, never permission to execute. Return concise safe explanation, never private reasoning or raw client content. Output grants no dispatch. Fixture results cannot establish live target facts.",
        "limits": {
            "accounting_window_ms": 3000,
            "calls_per_decision": 2,
            "cancel_grace_ms": 2000,
            "collection_entries": 64,
            "concurrency": 1,
            "configuration_bytes": 16384,
            "connect_time_ms": 5000,
            "context_bytes": 8192,
            "decision_wall_time_ms": 45000,
            "json_depth": 8,
            "per_call_wall_time_ms": 20000,
            "phase_wall_time_ms": 50000,
            "proposal_bytes": 8192,
            "repair_bytes": 2048,
            "request_bytes": 16384,
            "response_bytes": 65536,
            "response_envelope_json_depth": 32,
            "shared_wall_time_ms": 40000,
            "text_field_bytes": 1024
        },
        "method": "POST",
        "omissions": [
            "temperature",
            "top_p"
        ],
        "profile": "deepseek-responses-autonomous-v1",
        "recipient": "https://api.deepseek.com/responses",
        "requested_settings": {
            "max_output_tokens": 4096,
            "model": "deepseek-flash",
            "reasoning": {
                "effort": "high"
            },
            "stream": false,
            "tool_choice": "none",
            "tools": []
        },
        "run_profile": "autonomous-dns-v1",
        "schema_version": "m1_inference_proposal_v1",
        "template_version": "m1-autonomous-inference-v1",
        "transport": {
            "compression": false,
            "http_response_policy": "bounded-http1.1-interim-then-final-200-no-upgrade-identity-only",
            "json_refusal_policy": "bounded-json-refusal-v2-envelope32-proposal8",
            "normal_certificate_hostname_validation": true,
            "proxy": false,
            "redirects": false,
            "retries": false
        }
    })
}

pub(super) fn bind(
    pack: &[u8],
    parsed: &Value,
    ids: &[String],
    repair: Option<&[&str]>,
) -> Result<BoundCall, &'static str> {
    let config = configuration(parsed, ids)?;
    let text = std::str::from_utf8(pack).map_err(|_| "request_encode")?;
    let mut body = config["requested_settings"].clone();
    body["instructions"] = config["instructions"].clone();
    body["input"] = input(text, repair)?;
    body["text"] = json!({"format":{"type":"json_schema","name":"m1_inference_proposal_v1","schema":config["schema"]}});
    let configuration = bytes(&config, 16384)?;
    let body = bytes(&body, 16384)?;
    Ok(BoundCall {
        pack: pack.to_vec(),
        digest: compute_binding(pack, &configuration, &body),
        configuration,
        body,
    })
}

fn configuration(parsed: &Value, ids: &[String]) -> Result<Value, &'static str> {
    let (mode, profile, recipient, adapter) = profile(parsed)?;
    let schema = autonomous_schema(parsed, ids)?;
    let mut config = template();
    config["execution_mode"] = json!(mode);
    config["run_profile"] = parsed["run_profile"].clone();
    config["profile"] = json!(profile);
    config["recipient"] = json!(recipient);
    config["adapter_release"] = json!(adapter);
    if profile == "deterministic-autonomous-v1" {
        config["requested_settings"]["model"] = json!(profile);
    }
    config["schema"] = schema;
    Ok(config)
}

type Profile = (&'static str, &'static str, &'static str, &'static str);
fn profile(p: &Value) -> Result<Profile, &'static str> {
    match (p["run_profile"].as_str(), p["execution_mode"].as_str()) {
        (Some("autonomous-dns-fixture-v1"), Some("controlled_fixture")) => Ok((
            "controlled_fixture",
            "deterministic-autonomous-v1",
            "urn:duskweave:fixture:inference:v1",
            "deterministic-autonomous-v1",
        )),
        (Some("autonomous-dns-provider-fixture-v1"), Some("controlled_fixture")) => Ok((
            "controlled_fixture",
            "deepseek-responses-autonomous-v1",
            "https://api.deepseek.com/responses",
            "autonomous-libcurl-v1",
        )),
        (Some("autonomous-dns-v1"), Some("live")) => Ok((
            "live",
            "deepseek-responses-autonomous-v1",
            "https://api.deepseek.com/responses",
            "autonomous-libcurl-v1",
        )),
        _ => Err("request_encode"),
    }
}

fn autonomous_schema(p: &Value, ids: &[String]) -> Result<Value, &'static str> {
    let mut schema = schema(ids);
    let refs: Vec<_> = p["manifests"]
        .as_array()
        .ok_or("request_encode")?
        .iter()
        .map(|m| m["reference"].as_str().ok_or("request_encode"))
        .collect::<Result<_, _>>()?;
    if !refs.is_empty() {
        schema["properties"]["next_intent"]["anyOf"][1]["properties"]["manifest_reference"] =
            json!({"anyOf":[{"type":"null"},{"type":"string","enum":refs}]});
    }
    Ok(schema)
}
