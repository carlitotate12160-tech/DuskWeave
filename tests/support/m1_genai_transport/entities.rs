//! Private SDK entities controls with the same client and fixture.
use super::errors::assert_safe_error;
use super::*;

pub(super) fn response_caps_cover_success_error_chunked_and_close_delimited_entities() {
    for (status, framing) in [(200, 0), (503, 0), (200, 1), (200, 2)] {
        let body = vec![b' '; 65537];
        let server = Server::start("127.0.0.1:0", move |_| match framing {
            1 => Reply::Chunked(body.clone(), 257),
            2 => Reply::Close(body.clone()),
            _ => Reply::Length(status, body.clone()),
        });
        let result = observed(&server, "bounded", false);
        assert_eq!(result.evidence.http_status, Some(status));
        assert!(
            result
                .evidence
                .response_entity_bytes
                .is_some_and(|n| n > 65536)
        );
        assert!(result.evidence.raw_response.is_none());
        assert_eq!(
            classify_error(result.result.as_ref().err().unwrap()),
            "provider_response_limit"
        );
        assert_eq!(server.count(), 1);
    }
}
pub(super) fn exact_cap_status_invalid_json_redirect_and_reset_preserve_safe_evidence() {
    let mut exact = success_body();
    exact.resize(65536, b' ');
    let server = Server::start("127.0.0.1:0", move |_| Reply::Length(200, exact.clone()));
    let result = observed(&server, "exact", false);
    assert!(result.result.is_ok());
    assert_eq!(result.evidence.response_entity_bytes, Some(65536));
    drop(server);
    for status in [400, 429, 503, 302] {
        let server = Server::start("127.0.0.1:0", move |_| {
            Reply::Length(status, b"synthetic-provider-error-571".to_vec())
        });
        let result = observed(&server, "context", true);
        assert_eq!(result.evidence.http_status, Some(status));
        assert_safe_error(result, &["synthetic-provider-error-571"], false);
        assert_eq!(server.count(), 1);
    }
    for reply in [
        Reply::Length(200, b"{synthetic-malformed-571".to_vec()),
        Reply::Misleading(b"{}".to_vec(), 50),
        Reply::Reset,
    ] {
        let mut once = Some(reply);
        let server = Server::start("127.0.0.1:0", move |_| once.take().expect("no retry"));
        assert_safe_error(
            observed(&server, "context", false),
            &["synthetic-malformed-571"],
            false,
        );
        assert_eq!(server.count(), 1);
    }
}
pub(super) fn fragmented_request_waits_for_remaining_bytes_without_closing_the_connection() {
    use std::io::{Read, Write};
    let server = Server::start("127.0.0.1:0", |_| Reply::Length(200, b"{}".to_vec()));
    let mut client = std::net::TcpStream::connect(server.address).unwrap();
    client
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    client
        .write_all(b"POST /responses HTTP/1.1\r\nContent-Length: 2\r\n")
        .unwrap();
    let mut byte = [0];
    let read = client.read(&mut byte);
    assert!(
        matches!(read,Err(ref e) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)),
        "fixture must wait for remaining request bytes: wire={:?}",
        server.wire.lock().unwrap()
    );
    client.write_all(b"\r\n{}").unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reply = Vec::new();
    client.read_to_end(&mut reply).unwrap();
    assert!(reply.starts_with(b"HTTP/1.1 200"));
    assert_eq!(server.count(), 1);
    let wire = server.wire.lock().unwrap();
    assert_eq!(wire.read_complete, 1);
    assert_eq!(wire.write_complete, 1);
}

pub(super) fn exact_request_cap_and_first_illegal_byte_use_real_multibyte_serialization() {
    let server = Server::start("127.0.0.1:0", |_| Reply::Length(200, success_body()));
    let baseline = observed(&server, "base", false);
    let overhead = baseline.evidence.request_entity_bytes.unwrap() - 4;
    let content_bytes = 16384 - overhead;
    let exact = "\u{754c}".repeat(content_bytes / 3) + &"x".repeat(content_bytes % 3);
    let result = observed(&server, &exact, false);
    assert!(result.result.is_ok());
    assert_eq!(result.evidence.request_entity_bytes, Some(16384));
    assert_eq!(
        server.requests.lock().unwrap().last().unwrap().body.len(),
        16384
    );
    let illegal = observed(&server, &(exact + "x"), false);
    assert_eq!(illegal.evidence.request_entity_bytes, Some(16385));
    assert_eq!(
        classify_error(illegal.result.as_ref().err().unwrap()),
        "request_size"
    );
    assert_eq!(server.count(), 2, "first illegal byte must prevent send");
}

pub(super) fn maximum_pack_and_eight_repair_codes_use_observed_sdk_entity_bytes() {
    use crate::m1_reasoning::inference::{Reservation, Scope};
    use uuid::Uuid;
    // Synthetic identities qualify the serializer only. Actual reserved identity
    // attachment and restricted authority are exercised by the PG target.
    let scope = Scope {
        engagement: Uuid::from_u128(1),
        campaign: Uuid::from_u128(2),
        decision: Uuid::from_u128(3),
        permission: Uuid::from_u128(4),
        mission_revision: 1,
    };
    let end = Instant::now() + Duration::from_secs(4);
    let r = Reservation {
        generation: i64::MAX as u64,
        generation_call: Uuid::from_u128(5),
        repair_call: Uuid::from_u128(6),
        shared_deadline: end,
    };
    for name in [
        "deepseek-genai-responses-low-v1",
        "fixture-genai-responses-v1",
        "deepseek-genai-v4-pro-low-v1",
    ] {
        let profile = profiles::lookup(name).unwrap();
        let p = json!({"version":"m1_context_pack_autonomous_v3","run_profile":"autonomous-dns-fixture-v2","execution_mode":"controlled_fixture",
            "inference_profile":name,"engagement_id":scope.engagement,"campaign_id":scope.campaign,"decision_id":scope.decision,
            "permission_id":scope.permission,"mission_revision":1,"references":[{"id":"asset_0"},{"id":"mission"}],"manifests":[{"reference":"dns_udp"}],
            "question":"界 \"synthetic\" request"});
        for size in [0, 8192] {
            let mut pack = serde_json::to_vec(&p).unwrap();
            if size != 0 {
                pack.resize(size, b' ');
            }
            let upper = inference_intent::preflight(&pack, 8192, 16384).unwrap();
            let mut illegal = pack.clone();
            illegal.resize(8193, b' ');
            assert_eq!(
                inference_intent::preflight(&illegal, 8192, 16384),
                Err("request_size")
            );
            let server = Server::start("127.0.0.1:0", move |r| {
                let wire: Value = serde_json::from_slice(&r.body).unwrap();
                let pack = fixture::pack_in(&wire).unwrap();
                let mut envelope = fixture::envelope(&fixture::proposal(&pack));
                envelope["model"] = json!(profile.settings.model);
                Reply::Length(200, serde_json::to_vec(&envelope).unwrap())
            });
            for (call, codes) in [
                (r.generation_call, None),
                (r.repair_call, Some(&["unexpected_view_request"; 8][..])),
            ] {
                let bound = inference_intent::bind(&scope, &r, call, &pack, codes).unwrap();
                assert!(bound.configuration.len() <= upper);
                let intent = inference_intent::verify(&bound).unwrap();
                let key = "synthetic-private-profile-key";
                let mut prepared =
                    prepare_with_key(&bound, &intent, Duration::from_secs(3), profile, key.into())
                        .unwrap();
                assert_eq!(prepared.target.model.adapter_kind, AdapterKind::OpenAIResp);
                assert_eq!(&*prepared.target.model.model_name, profile.settings.model);
                assert_eq!(
                    format!("{}responses", prepared.target.endpoint.base_url()),
                    profile.connection.recipient
                );
                assert_eq!(prepared.requested_model, profile.settings.model);
                // No production endpoint/auth override exists; injection stays on this private object.
                prepared.target.endpoint =
                    Endpoint::from_owned(format!("http://{}/", server.address));
                prepared.target.auth = AuthData::from_single(key.to_owned());
                let start = Instant::now();
                let permit = Permit::new(start + Duration::from_secs(3));
                assert!(permit.publish(start, start + Duration::from_secs(3)));
                let observed = exchange(prepared, permit, start + Duration::from_secs(3)).unwrap();
                let bytes = observed.0.evidence.request_entity_bytes.unwrap();
                assert!(bytes <= 16384);
                let requests = server.requests.lock().unwrap();
                let request = requests.last().unwrap();
                assert!(request.headers.starts_with("POST /responses "));
                assert_eq!(request.body.len(), bytes);
                let wire: Value = serde_json::from_slice(&request.body).unwrap();
                assert_eq!(wire["model"], intent["model"]);
                assert_eq!(
                    wire["reasoning"]["effort"],
                    intent["settings"]["reasoning_effort"]
                );
                assert_eq!(
                    wire["max_output_tokens"],
                    intent["settings"]["max_output_tokens"]
                );
                assert_eq!(wire["store"], false);
                assert_eq!(wire["stream"], false);
                for field in ["temperature", "top_p", "tools", "tool_choice"] {
                    assert!(wire.get(field).is_none());
                }
                assert!(wire.to_string().contains("m1_inference_proposal_v1"));
                if codes.is_some() {
                    assert!(wire.to_string().contains("unexpected_view_request"));
                }
                drop(requests);
                eprintln!(
                    "safe_sdk_boundary profile={name} role={} pack={} intent={} upper={upper} request_entity={bytes} elapsed_ms={}",
                    intent["call_role"],
                    pack.len(),
                    bound.configuration.len(),
                    start.elapsed().as_millis()
                );
                let mut adapter = provider();
                let received = adapter
                    .admit(
                        observed.0,
                        &observed.1,
                        observed.2,
                        start.elapsed(),
                        &mut Basis,
                    )
                    .unwrap();
                assert!(matches!(
                    received.admission,
                    crate::m1_reasoning::proposal::Admission::Accepted(_)
                ));
                assert_eq!(received.usage.requested_model, profile.settings.model);
                assert_eq!(
                    received.usage.reported_model.as_deref(),
                    Some(profile.settings.model)
                );
                for field in ["model", "protocol", "recipient", "settings"] {
                    let mut changed = intent.clone();
                    changed[field] = json!("tampered");
                    changed.sort_all_objects();
                    let configuration = serde_json::to_vec(&changed).unwrap();
                    let digest = inference_intent::digest(&pack, &configuration);
                    let changed = BoundCall {
                        pack: pack.clone(),
                        configuration,
                        digest,
                        body: Vec::new(),
                    };
                    let mut adapter = provider();
                    assert_eq!(
                        adapter.prepare(&changed, Instant::now() + Duration::from_secs(3)),
                        Err("inference_decision_integrity")
                    );
                }
            }
            assert_eq!(server.count(), 2);
        }
        for case in [
            "invalid_proposal",
            "incomplete",
            "unknown_model",
            "decode_failure",
        ] {
            let mut envelope = fixture::envelope(&fixture::proposal(&p));
            envelope["model"] = json!(profile.settings.model);
            match case {
                "invalid_proposal" => envelope["output"][0]["content"][0]["text"] = json!("{}"),
                "incomplete" => {
                    envelope["status"] = json!("incomplete");
                    envelope["incomplete_details"] = json!({"reason":"max_output_tokens"});
                }
                "unknown_model" => envelope["model"] = json!("private-unrecognized-provider-model"),
                _ => envelope["output"][0]["content"] = json!(false),
            }
            let body = serde_json::to_vec(&envelope).unwrap();
            let server = Server::start("127.0.0.1:0", move |_| Reply::Length(200, body.clone()));
            let pack = serde_json::to_vec(&p).unwrap();
            let bound = inference_intent::bind(&scope, &r, r.generation_call, &pack, None).unwrap();
            let intent = inference_intent::verify(&bound).unwrap();
            let mut prepared = prepare_with_key(
                &bound,
                &intent,
                Duration::from_secs(3),
                profile,
                "synthetic-bound-model-key".into(),
            )
            .unwrap();
            prepared.target.endpoint = Endpoint::from_owned(format!("http://{}/", server.address));
            let start = Instant::now();
            let end = start + Duration::from_secs(3);
            let permit = Permit::new(end);
            assert!(permit.publish(start, end));
            let (observed, key, requested) = exchange(prepared, permit, end).unwrap();
            let mut adapter = provider();
            let result = adapter.admit(observed, &key, requested, start.elapsed(), &mut Basis);
            let report = adapter
                .diagnostic
                .report
                .as_ref()
                .expect("completed HTTP evidence retained");
            assert_eq!(report.usage.requested_model, profile.settings.model);
            if case == "unknown_model" {
                assert!(report.usage.reported_model.is_none());
            }
            if case == "incomplete" {
                assert!(matches!(result, Err("provider_incomplete")));
                assert!(matches!(
                    report.provider_response.incomplete_reason,
                    Some(crate::m1_reasoning::inference::IncompleteReason::MaxOutputTokens)
                ));
            }
            if case == "decode_failure" {
                assert!(matches!(result, Err("provider_protocol")));
            }
            if case == "invalid_proposal" {
                assert!(result.unwrap().admission.diagnostic().is_some());
            }
            assert_eq!(server.count(), 1);
        }
    }
}

fn measure_pro_v2_case(
    name: &str,
    pack_value: &Value,
    call: uuid::Uuid,
    codes: Option<&[&'static str]>,
) {
    use crate::m1_reasoning::inference::{Reservation, Scope};
    use uuid::Uuid;
    let profile = profiles::lookup(name).unwrap();
    let scope = Scope {
        engagement: Uuid::from_u128(11),
        campaign: Uuid::from_u128(12),
        decision: Uuid::from_u128(13),
        permission: Uuid::from_u128(14),
        mission_revision: 1,
    };
    let end = Instant::now() + Duration::from_millis(profile.timing.shared_ms);
    let r = Reservation {
        generation: i64::MAX as u64,
        generation_call: Uuid::from_u128(15),
        repair_call: Uuid::from_u128(16),
        shared_deadline: end,
    };
    let pack = serde_json::to_vec(pack_value).unwrap();
    let upper = inference_intent::preflight(&pack, 8192, 16384).unwrap();
    let bound = inference_intent::bind(&scope, &r, call, &pack, codes).unwrap();
    assert!(bound.configuration.len() <= upper);
    let intent = inference_intent::verify(&bound).unwrap();
    assert_eq!(intent["timing_profile"], profiles::PRO_V2_TIMING_PROFILE);
    assert_eq!(intent["settings"]["max_output_tokens"], 8192);
    let server = Server::start("127.0.0.1:0", move |req| {
        let wire: Value = serde_json::from_slice(&req.body).unwrap();
        let p = fixture::pack_in(&wire).unwrap();
        let mut envelope = fixture::envelope(&fixture::proposal(&p));
        envelope["model"] = json!(profile.settings.model);
        Reply::Length(200, serde_json::to_vec(&envelope).unwrap())
    });
    let key = if name == profiles::FIXTURE_PRO_V2_PROFILE {
        infrastructure_key(profile).unwrap()
    } else {
        "synthetic-pro-v2-key".to_owned()
    };
    let mut prepared =
        prepare_with_key(&bound, &intent, Duration::from_secs(3), profile, key).unwrap();
    prepared.target.endpoint = Endpoint::from_owned(format!("http://{}/", server.address));
    let start = Instant::now();
    let permit = Permit::new(end);
    assert!(permit.publish(start, end));
    let observed = exchange(prepared, permit, end).unwrap();
    let bytes = observed.0.evidence.request_entity_bytes.unwrap();
    assert!(bytes <= 16384);
    let requests = server.requests.lock().unwrap();
    let wire: Value = serde_json::from_slice(&requests.last().unwrap().body).unwrap();
    assert_eq!(wire["model"], "deepseek-v4-pro");
    assert_eq!(wire["reasoning"]["effort"], "low");
    assert_eq!(wire["max_output_tokens"], 8192);
    eprintln!(
        "safe_pro_v2_wire profile={name} role={} pack={} intent={} upper={upper} request_entity={bytes}",
        intent["call_role"],
        pack.len(),
        bound.configuration.len()
    );
}

pub(super) fn pro_v2_and_mirror_pack_intent_wire_and_clock_measurements() {
    use crate::m1_reasoning::inference::{Reservation, Scope};
    use uuid::Uuid;
    let mirror = profiles::lookup(profiles::FIXTURE_PRO_V2_PROFILE).unwrap();
    assert_eq!(
        infrastructure_key(mirror).as_deref(),
        Ok("duskweave-synthetic-sdk-fixture-key")
    );
    let eight_candidates: Vec<Value> = (0..8)
        .map(|i| json!({"reference": format!("asset_{i}"), "priority": "high", "note": "界 \"s\""}))
        .collect();
    for name in [profiles::PRO_V2_PROFILE, profiles::FIXTURE_PRO_V2_PROFILE] {
        let min_pack = json!({"version":"m1_context_pack_autonomous_v3","run_profile":"autonomous-dns-fixture-v2","execution_mode":"controlled_fixture",
            "inference_profile":name,"engagement_id":Uuid::from_u128(11),"campaign_id":Uuid::from_u128(12),"decision_id":Uuid::from_u128(13),
            "permission_id":Uuid::from_u128(14),"mission_revision":1,"references":[{"id":"asset_0"},{"id":"mission"}],"manifests":[{"reference":"dns_udp"}],
            "question":"minimum"});
        measure_pro_v2_case(name, &min_pack, Uuid::from_u128(15), None);
        let mut wide_pack = min_pack.clone();
        wide_pack["candidates"] = Value::Array(eight_candidates.clone());
        wide_pack["predecessor"] =
            json!({"hypothesis_summary":"界 \"prior\"","result_ref":Uuid::from_u128(99)});
        measure_pro_v2_case(
            name,
            &wide_pack,
            Uuid::from_u128(16),
            Some(&["unexpected_view_request"; 8]),
        );
    }
    let scope = Scope {
        engagement: Uuid::from_u128(21),
        campaign: Uuid::from_u128(22),
        decision: Uuid::from_u128(23),
        permission: Uuid::from_u128(24),
        mission_revision: 1,
    };
    let end = Instant::now() + Duration::from_millis(mirror.timing.shared_ms);
    let r = Reservation {
        generation: 1,
        generation_call: Uuid::from_u128(25),
        repair_call: Uuid::from_u128(26),
        shared_deadline: end,
    };
    let pack = serde_json::to_vec(&json!({"version":"m1_context_pack_autonomous_v3","run_profile":"autonomous-dns-fixture-v2","execution_mode":"controlled_fixture",
        "inference_profile":profiles::FIXTURE_PRO_V2_PROFILE,"engagement_id":scope.engagement,"campaign_id":scope.campaign,"decision_id":scope.decision,
        "permission_id":scope.permission,"mission_revision":1,"references":[{"id":"asset_0"},{"id":"mission"}],"manifests":[{"reference":"dns_udp"}],
        "question":"clock"})).unwrap();
    let bound = inference_intent::bind(&scope, &r, r.generation_call, &pack, None).unwrap();
    let mut sdk = provider();
    assert!(
        sdk.prepare(&bound, Instant::now() + Duration::from_millis(121999))
            .is_err()
    );
    let mut sdk = SdkProvider::new(Permit::new(end), || Ok(()));
    assert!(sdk.permit.publish(Instant::now(), end));
    sdk.prepare(&bound, end).unwrap();
    let clock = sdk.clock.take().unwrap();
    assert_eq!(clock.soft - clock.origin, Duration::from_millis(120000));
    assert_eq!(clock.hard - clock.origin, Duration::from_millis(122000));
}
