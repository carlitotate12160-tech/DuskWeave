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
    let p = json!({"version":"m1_context_pack_autonomous_v3","run_profile":"autonomous-dns-fixture-v2","execution_mode":"controlled_fixture",
        "inference_profile":"fixture-genai-responses-v1","engagement_id":scope.engagement,"campaign_id":scope.campaign,"decision_id":scope.decision,
        "permission_id":scope.permission,"mission_revision":1,"references":[{"id":"asset_0"},{"id":"mission"}],"manifests":[{"reference":"dns_udp"}],
        "question":"\u{754c} \"synthetic\" request"});
    let mut pack = serde_json::to_vec(&p).unwrap();
    pack.resize(8192, b' ');
    let upper = inference_intent::preflight(&pack, 8192, 16384).unwrap();
    let server = Server::start("127.0.0.1:0", |_| Reply::Length(200, success_body()));
    for (call, codes) in [
        (r.generation_call, None),
        (r.repair_call, Some(&["unexpected_view_request"; 8][..])),
    ] {
        let bound = inference_intent::bind(&scope, &r, call, &pack, codes).unwrap();
        assert!(bound.configuration.len() <= upper);
        let intent = inference_intent::verify(&bound).unwrap();
        let mut prepared = prepare_call(&bound, &intent, Duration::from_secs(3)).unwrap();
        // Endpoint injection is private to this test module, never production API.
        prepared.target.endpoint = Endpoint::from_owned(format!("http://{}/", server.address));
        let observed = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(prepared.client.exec_chat_observed(
                ModelSpec::Target(prepared.target),
                prepared.request,
                Some(&prepared.options),
            ));
        assert!(observed.result.is_ok());
        let bytes = observed.evidence.request_entity_bytes.unwrap();
        assert!(bytes <= 16384);
        assert_eq!(
            server.requests.lock().unwrap().last().unwrap().body.len(),
            bytes
        );
        let role = if codes.is_some() {
            "repair"
        } else {
            "generation"
        };
        let observation = format!(
            "safe_sdk_boundary role={} pack={} intent={} upper={} repair_codes={} request_entity={} response_entity={:?}",
            role,
            pack.len(),
            bound.configuration.len(),
            upper,
            codes.map_or(0, |c| c.len()),
            bytes,
            observed.evidence.response_entity_bytes
        );
        assert!(observation.starts_with(&format!("safe_sdk_boundary role={role} pack=")));
        for id in [
            scope.engagement,
            scope.campaign,
            scope.decision,
            scope.permission,
            r.generation_call,
            r.repair_call,
        ] {
            assert!(
                !observation.contains(&id.to_string()),
                "closed boundary observation excludes identities"
            );
        }
    }
    assert_eq!(server.count(), 2);
}
