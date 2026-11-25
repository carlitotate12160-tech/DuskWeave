use crate::lab_support::{
    sample_dns_request, sample_https_ipc_request, sample_https_safe_result,
    sample_lab_context_pack, sample_live_binding, uid,
};
use duskweave::m1_https::codec::{
    HttpsFrameReader, HttpsFrameWriter, decode_payload, encode_frame,
};
use duskweave::m1_https::ipc::{
    HTTPS_IPC_MAX_CONTROL, HTTPS_IPC_MAX_FRAME, HTTPS_IPC_MAX_FRAMES, HttpsAck, HttpsIpcRequest,
    HttpsOutcome, HttpsPhase, HttpsPhaseReady, HttpsPhaseStart,
};
use duskweave::m1_https::{
    HttpsOutcomeKind, HttpsSourceKind, RedirectDisposition, VERSION_HTTPS_LOOP,
};
use duskweave::m1_inference_profile::{
    FIXTURE_PRO_V2_PROFILE, HTTPS_LAB_HOSTNAME, HTTPS_LAB_PURPOSE, HTTPS_LAB_RUN_PROFILE,
};
use duskweave::m1_reasoning::inference::{Reservation, Scope};
use duskweave::m1_reasoning::inference_intent;
use serde_json::{Value, json};
use std::io::Cursor;

#[test]
fn map_a_https_lab_context_pack_and_canonical_intent_byte_ceilings() {
    let (eng, cmp, dec, perm, qref) = (uid(1), uid(2), uid(3), uid(4), uid(5));
    let pack = sample_lab_context_pack(eng, cmp, dec, perm, qref);
    let pack_bytes = serde_json::to_vec(&pack).unwrap();
    assert!(pack_bytes.len() <= 8192, "pack len={}", pack_bytes.len());

    let upper = inference_intent::preflight(&pack_bytes, 8192, 16384).unwrap();
    assert!(upper <= 16384, "upper={upper}");

    let scope = Scope {
        engagement: eng,
        campaign: cmp,
        decision: dec,
        permission: perm,
        mission_revision: 1,
    };
    let reservation = Reservation {
        generation: 1,
        generation_call: uid(0x11),
        repair_call: uid(0x12),
        shared_deadline: std::time::Instant::now(),
    };

    let bound_gen = inference_intent::bind(
        &scope,
        &reservation,
        reservation.generation_call,
        &pack_bytes,
        None,
    )
    .unwrap();
    assert!(bound_gen.configuration.len() <= upper);
    let gen_json = inference_intent::verify(&bound_gen).unwrap();
    inference_intent::verify_owned(
        &bound_gen,
        &scope,
        &reservation,
        reservation.generation_call,
    )
    .unwrap();
    assert_eq!(gen_json["run_profile"], HTTPS_LAB_RUN_PROFILE);
    assert_eq!(gen_json["execution_mode"], "live");
    assert_eq!(gen_json["profile"], FIXTURE_PRO_V2_PROFILE);
    assert_eq!(gen_json["qualification_purpose"], HTTPS_LAB_PURPOSE);
    assert_eq!(gen_json["live_binding"], sample_live_binding(qref));
    assert_eq!(gen_json["call_role"], "generation");

    let max_codes = ["unexpected_view_request"; 8];
    let bound_rep = inference_intent::bind(
        &scope,
        &reservation,
        reservation.repair_call,
        &pack_bytes,
        Some(&max_codes),
    )
    .unwrap();
    assert!(bound_rep.configuration.len() <= upper);
    let rep_json = inference_intent::verify(&bound_rep).unwrap();
    inference_intent::verify_owned(&bound_rep, &scope, &reservation, reservation.repair_call)
        .unwrap();
    assert_eq!(rep_json["call_role"], "repair");
    assert_eq!(rep_json["repair_error_codes"].as_array().unwrap().len(), 8);

    // Tampered configuration or digest refuses.
    let bad_bound = duskweave::m1_reasoning::request::BoundCall {
        pack: bound_gen.pack.clone(),
        digest: "0".repeat(64),
        configuration: bound_gen.configuration.clone(),
        body: Vec::new(),
    };
    assert!(inference_intent::verify(&bad_bound).is_err());

    // Fixture rejects explicit null or live fields; v4 rejects HTTPS LAB profile.
    let mut fix = pack.clone();
    fix["run_profile"] = json!(VERSION_HTTPS_LOOP);
    fix["execution_mode"] = json!("controlled_fixture");
    fix["live_binding"] = Value::Null;
    fix.as_object_mut().unwrap().remove("qualification_purpose");
    assert!(inference_intent::preflight(&serde_json::to_vec(&fix).unwrap(), 8192, 16384).is_err());

    let mut v4_alias = pack.clone();
    v4_alias["version"] = json!("m1_context_pack_autonomous_v4");
    assert!(
        inference_intent::preflight(&serde_json::to_vec(&v4_alias).unwrap(), 8192, 16384).is_err()
    );
}

#[test]
fn f1_https_ipc_request_separates_capability_profile_from_inference_profile() {
    let now = 1_700_000_000_000i64;
    let req = sample_https_ipc_request(now);
    req.validate(now).unwrap();

    let raw_val: Value = serde_json::to_value(&req).unwrap();
    assert_eq!(raw_val.as_object().unwrap().len(), 36);
    assert_eq!(raw_val["profile"], "https-head-v1");

    // Placing inference profile or DNS profile in capability profile field must fail.
    for bad_profile in [FIXTURE_PRO_V2_PROFILE, "dns-routing-v2", ""] {
        let mut r = req.clone();
        r.profile = bad_profile.into();
        assert!(r.validate_shape().is_err(), "accepted {bad_profile}");
    }

    for mut bad in [
        {
            let mut r = req.clone();
            r.execution_mode = "controlled_fixture".into();
            r
        },
        {
            let mut r = req.clone();
            r.run_profile = VERSION_HTTPS_LOOP.into();
            r
        },
        {
            let mut r = req.clone();
            r.hostname = "other.duskweave.test".into();
            r
        },
        {
            let mut r = req.clone();
            r.ipv4 = "198.51.100.11".into();
            r
        },
        {
            let mut r = req.clone();
            r.port = 8443;
            r
        },
        {
            let mut r = req.clone();
            r.approved_path = "/health".into();
            r
        },
        {
            let mut r = req.clone();
            r.method = "GET".into();
            r
        },
        {
            let mut r = req.clone();
            r.resolver_ipv4 = "1.1.1.1".into();
            r
        },
        {
            let mut r = req.clone();
            r.limits.header_bytes = 16384;
            r
        },
    ] {
        assert!(bad.validate(now).is_err());
        bad.sequence = 1;
        assert!(bad.validate_shape().is_err());
    }

    // Unknown or duplicate JSON fields in wire payload must refuse.
    let mut with_extra = raw_val.clone();
    with_extra["inference_profile"] = json!(FIXTURE_PRO_V2_PROFILE);
    assert!(
        decode_payload::<HttpsIpcRequest>(
            &serde_json::to_vec(&with_extra).unwrap(),
            HTTPS_IPC_MAX_FRAME
        )
        .is_err()
    );
}

#[test]
fn f3_closed_phase_status_matrix_and_head_result_projection() {
    let now = 1_700_000_000_000i64;
    let req = sample_https_ipc_request(now);
    let obs = now + 1200;

    // 1. HeadResponse (200 and 302): valid only at Head phase.
    let res_200 =
        sample_https_safe_result(&req, HttpsOutcome::HeadResponse, obs, 200, "none", None);
    res_200
        .validate_for(&req, HttpsPhase::Head, obs + 10)
        .unwrap();
    assert!(
        res_200
            .validate_for(&req, HttpsPhase::Connect, obs + 10)
            .is_err()
    );
    assert!(
        res_200
            .validate_for(&req, HttpsPhase::Tls, obs + 10)
            .is_err()
    );
    let head_200 = res_200.into_head_result(&req).unwrap();
    assert_eq!(head_200.outcome, HttpsOutcomeKind::HeadResponse);
    assert_eq!(head_200.status_code, Some(200));
    assert_eq!(head_200.redirect, RedirectDisposition::None);
    assert_eq!(
        head_200.source_kind,
        HttpsSourceKind::IssuedHttpsAcquisition
    );

    let res_302 =
        sample_https_safe_result(&req, HttpsOutcome::HeadResponse, obs, 302, "none", None);
    res_302
        .validate_for(&req, HttpsPhase::Head, obs + 10)
        .unwrap();
    let head_302 = res_302.into_head_result(&req).unwrap();
    assert_eq!(head_302.redirect, RedirectDisposition::RefusedRedirect);

    // 1xx or zero header_bytes or non-none TLS category on HeadResponse refuses.
    let mut bad_101 = res_200.clone();
    bad_101.status_code = 101;
    assert!(
        bad_101
            .validate_for(&req, HttpsPhase::Head, obs + 10)
            .is_err()
    );
    let mut bad_zero_hdr = res_200.clone();
    bad_zero_hdr.header_bytes = 0;
    assert!(
        bad_zero_hdr
            .validate_for(&req, HttpsPhase::Head, obs + 10)
            .is_err()
    );

    // 2. ConnectionFailed: valid at Connect OR Head, refused at Tls.
    let conn_fail =
        sample_https_safe_result(&req, HttpsOutcome::ConnectionFailed, obs, 0, "none", None);
    conn_fail
        .validate_for(&req, HttpsPhase::Connect, obs + 10)
        .unwrap();
    conn_fail
        .validate_for(&req, HttpsPhase::Head, obs + 10)
        .unwrap();
    assert!(
        conn_fail
            .validate_for(&req, HttpsPhase::Tls, obs + 10)
            .is_err()
    );
    assert_eq!(
        conn_fail.into_head_result(&req).unwrap().outcome,
        HttpsOutcomeKind::ConnectionFailed
    );

    // 3. TlsFailed: valid only at Tls with matching closed category and subcode.
    for (cat, lim) in [
        ("unknown_root", "tls_unknown_root"),
        ("hostname_mismatch", "tls_hostname_mismatch"),
        ("expired_or_not_yet_valid", "tls_expired_or_not_yet_valid"),
        ("protocol", "tls_protocol"),
        ("other_validation", "tls_other_validation"),
    ] {
        let tls_fail =
            sample_https_safe_result(&req, HttpsOutcome::TlsFailed, obs, 0, cat, Some(lim));
        tls_fail
            .validate_for(&req, HttpsPhase::Tls, obs + 10)
            .unwrap();
        assert!(
            tls_fail
                .validate_for(&req, HttpsPhase::Connect, obs + 10)
                .is_err()
        );
        assert!(
            tls_fail
                .validate_for(&req, HttpsPhase::Head, obs + 10)
                .is_err()
        );
        let mut wrong_lim = tls_fail.clone();
        wrong_lim.limitations[2] = "tls_unknown_root".into();
        if cat != "unknown_root" {
            assert!(
                wrong_lim
                    .validate_for(&req, HttpsPhase::Tls, obs + 10)
                    .is_err()
            );
        }
    }

    // 4. TimedOut: valid at any granted phase.
    let timed_out = sample_https_safe_result(&req, HttpsOutcome::TimedOut, obs, 0, "none", None);
    for phase in [HttpsPhase::Connect, HttpsPhase::Tls, HttpsPhase::Head] {
        timed_out.validate_for(&req, phase, obs + 10).unwrap();
    }

    // 5. ExecutionRefused: phase-specific subcodes enforced.
    let plain_refused =
        sample_https_safe_result(&req, HttpsOutcome::ExecutionRefused, obs, 0, "none", None);
    for phase in [HttpsPhase::Connect, HttpsPhase::Tls, HttpsPhase::Head] {
        plain_refused.validate_for(&req, phase, obs + 10).unwrap();
    }
    for sub in ["network_byte_limit_exceeded", "unsupported_http_response"] {
        let r = sample_https_safe_result(
            &req,
            HttpsOutcome::ExecutionRefused,
            obs,
            0,
            "none",
            Some(sub),
        );
        assert!(r.validate_for(&req, HttpsPhase::Connect, obs + 10).is_err());
        r.validate_for(&req, HttpsPhase::Tls, obs + 10).unwrap();
        r.validate_for(&req, HttpsPhase::Head, obs + 10).unwrap();
    }
    for sub in ["http_header_limit_exceeded", "malformed_http_response"] {
        let r = sample_https_safe_result(
            &req,
            HttpsOutcome::ExecutionRefused,
            obs,
            0,
            "none",
            Some(sub),
        );
        assert!(r.validate_for(&req, HttpsPhase::Connect, obs + 10).is_err());
        assert!(r.validate_for(&req, HttpsPhase::Tls, obs + 10).is_err());
        r.validate_for(&req, HttpsPhase::Head, obs + 10).unwrap();
    }
}

#[test]
fn f4_io_cutoff_and_bounded_terminal_timeout_handling_window() {
    let now = 1_700_000_000_000i64;
    let req = sample_https_ipc_request(now);
    let deadline = req.deadline_at_ms;

    // Timeout observed inside (deadline, deadline + 1000] and received <= deadline + 1000 passes.
    for obs in [deadline, deadline + 1, deadline + 500, deadline + 1000] {
        let to = sample_https_safe_result(&req, HttpsOutcome::TimedOut, obs, 0, "none", None);
        to.validate_for(&req, HttpsPhase::Head, deadline + 1000)
            .unwrap();
        to.validate_historical_for(&req, HttpsPhase::Head).unwrap();
        assert_eq!(
            to.into_head_result(&req).unwrap().perceived_at,
            obs.div_euclid(1000)
        );
    }

    // Timeout beyond deadline + 1000 or received after handling window or in the future refuses.
    let to_late = sample_https_safe_result(
        &req,
        HttpsOutcome::TimedOut,
        deadline + 1001,
        0,
        "none",
        None,
    );
    assert!(
        to_late
            .validate_for(&req, HttpsPhase::Head, deadline + 1001)
            .is_err()
    );
    assert!(
        to_late
            .validate_historical_for(&req, HttpsPhase::Head)
            .is_err()
    );

    let to_valid = sample_https_safe_result(
        &req,
        HttpsOutcome::TimedOut,
        deadline + 200,
        0,
        "none",
        None,
    );
    assert!(
        to_valid
            .validate_for(&req, HttpsPhase::Head, deadline + 1001)
            .is_err()
    );
    assert!(
        to_valid
            .validate_for(&req, HttpsPhase::Head, deadline + 199)
            .is_err()
    );

    // Non-timeout result observed at deadline + 1 (after I/O cutoff) refuses.
    for outcome in [
        HttpsOutcome::HeadResponse,
        HttpsOutcome::ConnectionFailed,
        HttpsOutcome::ExecutionRefused,
    ] {
        let status = if outcome == HttpsOutcome::HeadResponse {
            200
        } else {
            0
        };
        let late_non_to =
            sample_https_safe_result(&req, outcome, deadline + 1, status, "none", None);
        assert!(
            late_non_to
                .validate_for(&req, HttpsPhase::Head, deadline + 10)
                .is_err()
        );
        assert!(
            late_non_to
                .validate_historical_for(&req, HttpsPhase::Head)
                .is_err()
        );
    }

    // Checked arithmetic overflow on deadline refuses safely.
    let mut overflow_req = req.clone();
    overflow_req.deadline_at_ms = i64::MAX;
    let res = sample_https_safe_result(&overflow_req, HttpsOutcome::TimedOut, now, 0, "none", None);
    assert!(
        res.validate_for(&overflow_req, HttpsPhase::Head, now)
            .is_err()
    );
}

#[test]
fn cross_language_wire_framing_and_sixteen_frame_cap() {
    let now = 1_700_000_000_000i64;
    let req = sample_https_ipc_request(now);
    let ready = HttpsPhaseReady::new(&req, HttpsPhase::Connect);
    let start = HttpsPhaseStart::new(&req, HttpsPhase::Connect, req.deadline_at_ms);
    let res = sample_https_safe_result(
        &req,
        HttpsOutcome::HeadResponse,
        now + 500,
        200,
        "none",
        None,
    );
    let ack = HttpsAck::new(&req);

    assert_eq!(
        serde_json::to_value(&res)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        13
    );
    assert_eq!(ack.sequence, 8);

    let framed_req = encode_frame(&req, HTTPS_IPC_MAX_FRAME).unwrap();
    let len = u32::from_be_bytes(framed_req[..4].try_into().unwrap()) as usize;
    assert_eq!(len + 4, framed_req.len());
    let decoded_req: HttpsIpcRequest =
        decode_payload(&framed_req[4..], HTTPS_IPC_MAX_FRAME).unwrap();
    assert_eq!(decoded_req, req);

    // 16-frame ceiling on HttpsFrameWriter and HttpsFrameReader.
    let mut sink = Vec::new();
    let mut writer = HttpsFrameWriter::new(&mut sink);
    for _ in 0..HTTPS_IPC_MAX_FRAMES {
        writer.start(&start).unwrap();
    }
    assert_eq!(writer.count(), 16);
    assert!(writer.start(&start).is_err());

    let mut ready_buf = Vec::new();
    for _ in 0..=HTTPS_IPC_MAX_FRAMES {
        ready_buf.extend_from_slice(&encode_frame(&ready, HTTPS_IPC_MAX_CONTROL).unwrap());
    }
    let mut reader = HttpsFrameReader::new(Cursor::new(ready_buf));
    for _ in 0..HTTPS_IPC_MAX_FRAMES {
        assert!(reader.ready().unwrap().matches(&req, HttpsPhase::Connect));
    }
    assert!(reader.ready().is_err());

    // DNS request singleton scope check.
    let dns = sample_dns_request(now);
    dns.validate(now).unwrap();
    assert_eq!(dns.name, HTTPS_LAB_HOSTNAME);
    assert_eq!(dns.allowed_names, vec![HTTPS_LAB_HOSTNAME]);
}
