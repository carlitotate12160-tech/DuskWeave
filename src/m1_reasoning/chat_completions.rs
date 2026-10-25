//! Bounded non-streaming Chat Completions codec; proposal semantics remain Cognition-owned.
use super::inference::{Received, Usage};
use super::proposal::{self, ProposalBasis};
use serde_json::{Value, json};
use std::time::Duration;

pub(super) fn request(
    body: &mut Value,
    config: &Value,
    pack: &str,
    repair: Option<&[&str]>,
) -> Result<(), &'static str> {
    let inputs = super::request::input(pack, repair)?;
    let mut messages = vec![json!({"role":"system","content":config["instructions"]})];
    if let Some(text) = inputs.as_str() {
        messages.push(json!({"role":"user","content":text}));
    } else {
        for input in inputs.as_array().ok_or("request_encode")? {
            messages.push(json!({"role":"user","content":input["content"]}));
        }
    }
    body["messages"] = json!(messages);
    body["response_format"] = json!({"type":"json_schema","json_schema":{
        "name":"m1_inference_proposal_v1","schema":config["schema"]}});
    Ok(())
}

pub fn admit_envelope(
    raw: &[u8],
    elapsed: Duration,
    basis: &mut dyn ProposalBasis,
) -> Result<Received, &'static str> {
    let v = super::bounded_json::parse_envelope_detailed(raw).map_err(|_| "provider_protocol")?;
    let choices = v["choices"].as_array().ok_or("provider_protocol")?;
    if choices.len() != 1 {
        return Err("provider_protocol");
    }
    let choice = &choices[0];
    let message = &choice["message"];
    let valid = [
        choice["index"] == 0,
        choice["finish_reason"] == "stop",
        message["role"] == "assistant",
        message.get("refusal").is_none_or(Value::is_null),
        message.get("tool_calls").is_none(),
        message.get("function_call").is_none(),
    ];
    if !valid.into_iter().all(|v| v) {
        return Err("provider_incomplete");
    }
    let text = message["content"].as_str().ok_or("provider_protocol")?;
    Ok(Received {
        admission: proposal::admit(text.as_bytes(), basis),
        accounting_confirmed: false,
        usage: Usage {
            input_tokens: v["usage"]["prompt_tokens"].as_u64(),
            output_tokens: v["usage"]["completion_tokens"].as_u64(),
            reasoning_tokens: v["usage"]["completion_tokens_details"]["reasoning_tokens"].as_u64(),
            response_bytes: raw.len(),
            wall_time_ms: elapsed.as_millis() as u64,
            requested_model: "protocol-fixture-v1",
            reported_model: v["model"]
                .as_str()
                .filter(|s| *s == "protocol-fixture-v1")
                .map(str::to_owned),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Basis;
    impl ProposalBasis for Basis {
        fn reference_eligible(&self, r: &str) -> bool {
            r == "asset_0"
        }
        fn manifest_eligible(&self, _: &str) -> bool {
            false
        }
        fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
            Ok(())
        }
        fn safe_explanation(&self, text: &str) -> bool {
            !text.contains("api_key")
        }
    }
    fn complete() -> Value {
        let p = json!({"version":"m1_inference_proposal_v1","hypothesis":"é routing hypothesis","considered_alternative":"unavailable",
            "references":["asset_0"],"expected_observation":"bounded observation","falsifying_observation":"missing premise",
            "uncertainty":"synthetic only","rationale":"no dispatch","next_intent":{"category":"retain","references":["asset_0"],"manifest_reference":null}});
        json!({"model":"unavailable-model-metadata","choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":p.to_string()}}]})
    }
    #[test]
    fn complete_text_is_screened_and_missing_usage_or_model_identity_remains_unknown() {
        let raw = serde_json::to_vec(&complete()).unwrap();
        let received = admit_envelope(&raw, Duration::ZERO, &mut Basis).unwrap();
        assert!(matches!(
            received.admission,
            proposal::Admission::Accepted(_)
        ));
        assert_eq!(received.usage.input_tokens, None);
        assert_eq!(received.usage.reported_model, None);
        let mut v = complete();
        v["usage"] = json!({"prompt_tokens":1,"completion_tokens":2,"completion_tokens_details":{"reasoning_tokens":1}});
        let received =
            admit_envelope(&serde_json::to_vec(&v).unwrap(), Duration::ZERO, &mut Basis).unwrap();
        assert_eq!(received.usage.input_tokens, Some(1));
        assert_eq!(received.usage.reasoning_tokens, Some(1));
    }
    #[test]
    fn ambiguous_incomplete_tools_nontext_and_unbounded_envelopes_are_refused() {
        for value in [json!({}), json!({"choices":[]}), json!({"choices":[{},{}]})] {
            assert!(
                admit_envelope(
                    &serde_json::to_vec(&value).unwrap(),
                    Duration::ZERO,
                    &mut Basis
                )
                .is_err()
            );
        }
        for (path, value) in [("finish_reason", json!("length")), ("index", json!(1))] {
            let mut v = complete();
            v["choices"][0][path] = value;
            assert!(
                admit_envelope(&serde_json::to_vec(&v).unwrap(), Duration::ZERO, &mut Basis)
                    .is_err()
            );
        }
        for (path, value) in [
            ("tool_calls", json!([])),
            ("function_call", json!({})),
            ("refusal", json!("refused")),
            ("role", json!("tool")),
            ("content", json!([])),
        ] {
            let mut v = complete();
            v["choices"][0]["message"][path] = value;
            assert!(
                admit_envelope(&serde_json::to_vec(&v).unwrap(), Duration::ZERO, &mut Basis)
                    .is_err()
            );
        }
        for raw in [
            vec![0xff],
            vec![b' '; 65537],
            b"{\"choices\":[],\"choices\":[]}".to_vec(),
        ] {
            assert!(admit_envelope(&raw, Duration::ZERO, &mut Basis).is_err());
        }
        let mut v = complete();
        let text = v["choices"][0]["message"]["content"]
            .as_str()
            .unwrap()
            .replace("é routing hypothesis", "api_key synthetic");
        v["choices"][0]["message"]["content"] = json!(text);
        let result =
            admit_envelope(&serde_json::to_vec(&v).unwrap(), Duration::ZERO, &mut Basis).unwrap();
        assert!(matches!(
            result.admission,
            proposal::Admission::Rejected("unsafe_explanation")
        ));
    }
}
