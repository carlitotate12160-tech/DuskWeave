//! Closed public HTTPS worker-boundary contract tests and Linux executable-pair qualification.
use duskweave::m1_https::codec::{decode_payload, encode_frame};
use duskweave::m1_https::ipc::{
    HTTPS_IPC_MAX_FRAME, HttpsIpcRequest, HttpsLimits, HttpsOutcome, HttpsPhase, HttpsSafeResult,
};
use duskweave::m1_https::result::{
    LAB_SCOPE_LIMITATION, PUBLIC_SCOPE_LIMITATION, valid_issued_limitations,
};
use duskweave::m1_https::{
    HTTPS_METHOD_HEAD, HTTPS_PORT, HttpsHeadResult, HttpsOutcomeKind, HttpsSourceKind,
    RedirectDisposition, VERSION_HEAD_RESULT, VERSION_HTTPS_IPC,
};
use duskweave::m1_inference_permission::LiveBinding;
use duskweave::m1_inference_profile::{
    HTTPS_HEAD_PROFILE, HTTPS_LAB_HOSTNAME, HTTPS_LAB_IPV4, HTTPS_LAB_RESOLVER,
    HTTPS_LAB_RUN_PROFILE, HTTPS_PUBLIC_RUN_PROFILE, PRO_V2_PROFILE, live_binding,
    resolver_binding, sdk_versions,
};
use duskweave::m1_reasoning::inference_intent;
use duskweave::m1_run::RunRequest;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::Path;
use uuid::Uuid;

#[path = "support/m1_https_lab.rs"]
mod lab_support;
#[cfg(target_os = "linux")]
#[path = "m1_https_public_worker/process_pair.rs"]
mod process_pair;

pub(crate) fn uid(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

pub(crate) fn sample_public_request(now_ms: i64) -> HttpsIpcRequest {
    HttpsIpcRequest {
        schema: VERSION_HTTPS_IPC.into(),
        kind: "REQUEST".into(),
        attempt: uid(0x901),
        session_generation: 1,
        sequence: 0,
        engagement_id: uid(0x902),
        campaign_id: uid(0x903),
        decision_id: uid(0x904),
        run_id: uid(0x905),
        step_index: 2,
        attempt_kind: "head".into(),
        result_id: uid(0x906),
        profile: HTTPS_HEAD_PROFILE.into(),
        execution_mode: "live".into(),
        run_profile: HTTPS_PUBLIC_RUN_PROFILE.into(),
        release_sha256: "b".repeat(64),
        qualification_ref: uid(0x907),
        asset_ref: uid(0x21),
        endpoint_ref: "endpoint_0".into(),
        hostname: "probe.example.invalid".into(),
        ipv4: "93.184.216.34".into(),
        port: HTTPS_PORT,
        approved_path: "/".into(),
        method: HTTPS_METHOD_HEAD.into(),
        parent_claim_id: uid(0x908),
        parent_claim_revision: 1,
        resolver_ipv4: "1.1.1.1".into(),
        vantage_ref: uid(0x909),
        permission_id: uid(0x90a),
        expected_mission_revision: 1,
        authority_evaluated_at_ms: now_ms - 100,
        claim_valid_until_s: (now_ms / 1000) + 60,
        episode_deadline_at_ms: now_ms + 55_000,
        deadline_at_ms: now_ms + 5000,
        max_duration_ms: 5000,
        limits: HttpsLimits::fixed(),
    }
}

fn sample_public_safe_result(
    req: &HttpsIpcRequest,
    outcome: HttpsOutcome,
    observed_at_ms: i64,
    status_code: u16,
    tls_category: &str,
    subcode: Option<&str>,
) -> HttpsSafeResult {
    let second = match outcome {
        HttpsOutcome::HeadResponse => "head_response_does_not_prove_access_or_objective_completion",
        HttpsOutcome::ConnectionFailed => "single_endpoint_connection_failed_not_whole_host",
        HttpsOutcome::TlsFailed => "single_endpoint_tls_failed_not_whole_host",
        HttpsOutcome::TimedOut => "single_endpoint_timeout_not_whole_host",
        HttpsOutcome::ExecutionRefused => "single_endpoint_execution_refused_not_whole_host",
    };
    let mut limitations = vec![PUBLIC_SCOPE_LIMITATION.into(), second.into()];
    if let Some(c) = subcode {
        limitations.push(c.into());
    }
    let (net, hdr, code) = if outcome == HttpsOutcome::HeadResponse {
        (512, 128, status_code)
    } else {
        (0, 0, 0)
    };
    let redirect = if outcome == HttpsOutcome::HeadResponse && (300..=399).contains(&status_code) {
        RedirectDisposition::RefusedRedirect
    } else {
        RedirectDisposition::None
    };
    HttpsSafeResult {
        schema: VERSION_HTTPS_IPC.into(),
        kind: "RESULT".into(),
        attempt: req.attempt,
        session_generation: req.session_generation,
        sequence: 7,
        outcome,
        observed_at_ms,
        network_bytes: net,
        header_bytes: hdr,
        status_code: code,
        tls_category: tls_category.into(),
        redirect,
        limitations,
    }
}

#[test]
fn public_https_worker_request_and_scope_projection_accepted() {
    let now = 1_700_000_000_000i64;
    let req = sample_public_request(now);
    req.validate(now).unwrap();
    let safe = sample_public_safe_result(
        &req,
        HttpsOutcome::ConnectionFailed,
        now + 100,
        0,
        "none",
        None,
    );
    safe.validate_for(&req, HttpsPhase::Connect, now + 150)
        .unwrap();
    let head = safe.into_head_result(&req).unwrap();
    assert_eq!(head.limitations[0], PUBLIC_SCOPE_LIMITATION);
}

#[test]
fn public_ipc_request_wire_bytes_and_closed_tuple_enforced() {
    let now = 1_700_000_000_000i64;
    let mut min_req = sample_public_request(now);
    min_req.step_index = 1;
    min_req.hostname = "a.b".into();
    min_req.ipv4 = "1.1.1.1".into();
    min_req.validate(now).unwrap();
    let min_framed = encode_frame(&min_req, HTTPS_IPC_MAX_FRAME).unwrap();
    assert!(min_framed.len() <= 4 + HTTPS_IPC_MAX_FRAME);
    let decoded_min: HttpsIpcRequest =
        decode_payload(&min_framed[4..], HTTPS_IPC_MAX_FRAME).unwrap();
    assert_eq!(decoded_min, min_req);

    let mut max_req = min_req.clone();
    max_req.session_generation = u64::MAX;
    max_req.step_index = 4;
    max_req.parent_claim_revision = u32::MAX;
    max_req.expected_mission_revision = u64::MAX;
    max_req.endpoint_ref = "endpoint_9999999".into(); // 16 bytes max
    max_req.hostname = format!(
        "{}.{}.{}.{}",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(61)
    );
    assert_eq!(max_req.hostname.len(), 253);
    max_req.ipv4 = "223.255.255.255".into();
    max_req.resolver_ipv4 = "223.255.255.254".into();
    max_req.validate(now).unwrap();
    let max_framed = encode_frame(&max_req, HTTPS_IPC_MAX_FRAME).unwrap();
    let max_payload_len = max_framed.len() - 4;
    assert!(max_payload_len <= HTTPS_IPC_MAX_FRAME);
    let decoded_max: HttpsIpcRequest =
        decode_payload(&max_framed[4..], HTTPS_IPC_MAX_FRAME).unwrap();
    assert_eq!(decoded_max, max_req);
    eprintln!(
        "WIRE_BYTES min_payload={} min_framed={} max_payload={} max_framed={} ceiling={}",
        min_framed.len() - 4,
        min_framed.len(),
        max_payload_len,
        max_framed.len(),
        HTTPS_IPC_MAX_FRAME
    );

    assert_eq!(
        serde_json::to_value(&min_req)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        36
    );
    for f in [
        (|r: &mut HttpsIpcRequest| r.approved_path = "/health".into()) as fn(&mut HttpsIpcRequest),
        |r| r.port = 8443,
        |r| r.method = "GET".into(),
        |r| r.execution_mode = "controlled_fixture".into(),
        |r| r.profile = PRO_V2_PROFILE.into(),
        |r| r.run_profile = "autonomous-dns-genai-live-v1".into(),
        |r| r.resolver_ipv4 = HTTPS_LAB_RESOLVER.into(),
        |r| r.run_profile = HTTPS_LAB_RUN_PROFILE.into(),
        |r| r.endpoint_ref = "endpoint_12345678".into(),
        |r| r.max_duration_ms = 5001,
        |r| r.deadline_at_ms = r.authority_evaluated_at_ms + 100,
        |r| r.limits.header_bytes = 8193,
    ] {
        let mut r = min_req.clone();
        f(&mut r);
        assert!(r.validate(now).is_err());
    }
    let mut overflow = min_req.clone();
    overflow.deadline_at_ms = i64::MAX - 10;
    overflow.episode_deadline_at_ms = i64::MAX;
    assert!(overflow.validate(now).is_err());
}

#[test]
fn public_ipc_request_hostname_and_special_ipv4_matrix() {
    let now = 1_700_000_000_000i64;
    let base = sample_public_request(now);
    let bad_hosts = "\
        localhost probe.example.invalid. .probe.example.invalid a..b.invalid \
        -a.example.invalid a-.example.invalid Probe.example.invalid \
        https://probe.example.invalid probe.example.invalid/path \
        93.184.216.34 ::1 bad_host.example.invalid";
    for bad_host in std::iter::once("")
        .chain(bad_hosts.split_whitespace())
        .chain([
            format!("{}.example.invalid", "a".repeat(64)).as_str(),
            format!(
                "{}.{}.{}.{}",
                "a".repeat(63),
                "b".repeat(63),
                "c".repeat(63),
                "d".repeat(62)
            )
            .as_str(),
        ])
    {
        let mut r = base.clone();
        r.hostname = bad_host.into();
        assert!(r.validate(now).is_err(), "accepted bad host {bad_host}");
    }
    let bad_ips = "\
        0.0.0.0 0.255.255.255 10.0.0.0 10.255.255.255 \
        100.64.0.0 100.127.255.255 127.0.0.0 127.0.0.1 127.255.255.255 \
        169.254.0.0 169.254.255.255 172.16.0.0 172.31.255.255 \
        192.0.0.0 192.0.0.255 192.0.2.0 192.0.2.53 192.0.2.255 \
        192.88.99.0 192.88.99.255 192.168.0.0 192.168.255.255 \
        198.18.0.0 198.19.255.255 198.51.100.0 198.51.100.10 198.51.100.255 \
        203.0.113.0 203.0.113.255 224.0.0.0 240.0.0.0 255.255.255.255 \
        093.184.216.34 ::1 not-an-ip";
    for bad_ip in bad_ips.split_whitespace() {
        let mut r1 = base.clone();
        r1.ipv4 = bad_ip.into();
        assert!(r1.validate(now).is_err(), "accepted bad target ip {bad_ip}");
        let mut r2 = base.clone();
        r2.resolver_ipv4 = bad_ip.into();
        assert!(
            r2.validate(now).is_err(),
            "accepted bad resolver ip {bad_ip}"
        );
    }
    let ok_ips = "\
        1.0.0.0 9.255.255.255 11.0.0.0 100.63.255.255 100.128.0.0 \
        126.255.255.255 128.0.0.0 172.15.255.255 172.32.0.0 \
        192.0.1.0 198.17.255.255 198.20.0.0 223.255.255.255";
    for ok_ip in ok_ips.split_whitespace() {
        let mut r = base.clone();
        r.ipv4 = ok_ip.into();
        r.resolver_ipv4 = ok_ip.into();
        r.validate(now).unwrap();
    }
}

#[test]
fn public_safe_result_scope_binding_and_cross_scope_refusal() {
    let now = 1_700_000_000_000i64;
    let pub_req = sample_public_request(now);
    let mut lab_req = pub_req.clone();
    lab_req.run_profile = HTTPS_LAB_RUN_PROFILE.into();
    lab_req.hostname = HTTPS_LAB_HOSTNAME.into();
    lab_req.ipv4 = HTTPS_LAB_IPV4.into();
    lab_req.resolver_ipv4 = HTTPS_LAB_RESOLVER.into();

    let pub_200 = sample_public_safe_result(
        &pub_req,
        HttpsOutcome::HeadResponse,
        now + 100,
        200,
        "none",
        None,
    );
    pub_200
        .validate_for(&pub_req, HttpsPhase::Head, now + 150)
        .unwrap();
    pub_200
        .validate_historical_for(&pub_req, HttpsPhase::Head)
        .unwrap();
    let head = pub_200.into_head_result(&pub_req).unwrap();
    assert_eq!(head.source_kind, HttpsSourceKind::IssuedHttpsAcquisition);
    head.validate().unwrap();

    // Historical validation has no current-clock freshness check.
    let old_req = sample_public_request(1_600_000_000_000);
    let old_res = sample_public_safe_result(
        &old_req,
        HttpsOutcome::ConnectionFailed,
        1_600_000_000_500,
        0,
        "none",
        None,
    );
    old_res
        .validate_historical_for(&old_req, HttpsPhase::Connect)
        .unwrap();

    // Cross-scope substitution must fail in validate_for, validate_historical_for, and direct into_head_result.
    for (res, req) in [
        (&pub_200, &lab_req),
        (
            &{
                let mut l = pub_200.clone();
                l.limitations[0] = LAB_SCOPE_LIMITATION.into();
                l
            },
            &pub_req,
        ),
    ] {
        assert!(res.validate_for(req, HttpsPhase::Head, now + 150).is_err());
        assert!(res.validate_historical_for(req, HttpsPhase::Head).is_err());
        assert!(res.into_head_result(req).is_err());
    }

    // Legacy valid_issued_limitations remains strictly LAB-only.
    let mut lab_200 = pub_200.clone();
    lab_200.limitations[0] = LAB_SCOPE_LIMITATION.into();
    assert!(valid_issued_limitations(
        HttpsOutcomeKind::HeadResponse,
        &lab_200.limitations
    ));
    assert!(!valid_issued_limitations(
        HttpsOutcomeKind::HeadResponse,
        &pub_200.limitations
    ));

    // Duplicated, mixed, unknown, or raw-containing limitations fail.
    let head_lim = "head_response_does_not_prove_access_or_objective_completion";
    for bad_lims in [
        vec![
            PUBLIC_SCOPE_LIMITATION.into(),
            PUBLIC_SCOPE_LIMITATION.into(),
            head_lim.into(),
        ],
        vec![
            LAB_SCOPE_LIMITATION.into(),
            head_lim.into(),
            PUBLIC_SCOPE_LIMITATION.into(),
        ],
        vec!["unknown_scope".into(), head_lim.into()],
        vec![
            PUBLIC_SCOPE_LIMITATION.into(),
            "https://evil.example".into(),
        ],
    ] {
        let mut r = pub_200.clone();
        r.limitations = bad_lims;
        assert!(
            r.validate_for(&pub_req, HttpsPhase::Head, now + 150)
                .is_err()
        );
        assert!(r.into_head_result(&pub_req).is_err());
        assert!(
            HttpsHeadResult {
                limitations: r.limitations,
                ..head.clone()
            }
            .validate()
            .is_err()
        );
    }
    let mut bad_unknown_req = pub_req.clone();
    bad_unknown_req.run_profile = "unknown".into();
    assert!(pub_200.into_head_result(&bad_unknown_req).is_err());
    let mut bad_ver = head.clone();
    bad_ver.version = VERSION_HEAD_RESULT.into();
    bad_ver.validate().unwrap();
}

#[test]
fn public_m1_run_and_inference_admission_remains_closed() {
    assert!(sdk_versions(HTTPS_PUBLIC_RUN_PROFILE).is_err());
    assert!(!resolver_binding(HTTPS_PUBLIC_RUN_PROFILE, "1.1.1.1"));
    assert!(!live_binding(
        HTTPS_PUBLIC_RUN_PROFILE,
        "live",
        PRO_V2_PROFILE
    ));
    let lb = LiveBinding {
        qualification_ref: uid(1),
        release_sha256: "a".repeat(64),
        resolver_ipv4: "1.1.1.1".into(),
    };
    assert!(lb.validate_for(HTTPS_PUBLIC_RUN_PROFILE).is_err());

    let (eng, cmp, qref, rel) = (uid(1), uid(2), uid(9), "a".repeat(64));
    let mut reg = lab_support::lab_registration_json(eng, cmp, qref, &rel, 1_700_000_060);
    // Baseline LAB fixture is valid before mutating to public.
    duskweave::input::parse_register(&serde_json::to_vec(&reg).unwrap()).unwrap();
    let lab_run: RunRequest = serde_json::from_value(lab_support::lab_run_request_json(
        &reg,
        uid(10),
        qref,
        &rel,
        1_700_000_600_000,
    ))
    .unwrap();
    lab_run.validate().unwrap();

    reg["m1_inference_permission"]["run_profile"] = json!(HTTPS_PUBLIC_RUN_PROFILE);
    reg["m1_inference_permission"]["profile"] = json!(PRO_V2_PROFILE);
    reg["m1_inference_permission"]["recipient"] = json!("https://api.deepseek.com/responses");
    reg["m1_inference_permission"]["live_binding"]["resolver_ipv4"] = json!("1.1.1.1");
    assert!(duskweave::input::parse_register(&serde_json::to_vec(&reg).unwrap()).is_err());

    let mut pub_run_v =
        lab_support::lab_run_request_json(&reg, uid(11), qref, &rel, 1_700_000_600_000);
    pub_run_v["run_profile"] = json!(HTTPS_PUBLIC_RUN_PROFILE);
    pub_run_v["inference_profile"] = json!(PRO_V2_PROFILE);
    pub_run_v["live_binding"]["resolver_ipv4"] = json!("1.1.1.1");
    pub_run_v["seed_names"][0]["name"] = json!("probe.example.invalid");
    let pub_run: RunRequest = serde_json::from_value(pub_run_v).unwrap();
    assert!(pub_run.validate().is_err());

    let perm_id = Uuid::parse_str(
        reg["m1_inference_permission"]["permission_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let mut pack = lab_support::sample_lab_context_pack(eng, cmp, uid(12), perm_id, qref);
    pack["run_profile"] = json!(HTTPS_PUBLIC_RUN_PROFILE);
    pack["inference_profile"] = json!(PRO_V2_PROFILE);
    pack["qualification_purpose"] = json!("LIVE");
    pack["live_binding"]["resolver_ipv4"] = json!("1.1.1.1");
    assert!(inference_intent::preflight(&serde_json::to_vec(&pack).unwrap(), 8192, 16384).is_err());
}

#[test]
fn vendored_public_roots_provenance_and_dockerfile_match() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pem = std::fs::read(root.join("boundary/trust/cacert-2026-11-05.pem")).unwrap();
    assert_eq!(pem.len(), 200_116);
    assert!(!pem.contains(&b'\r'));
    let digest: String = Sha256::digest(&pem)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        digest,
        "427c064216761a1e72a8b30f0d954d08cb6e4435a82091fb865afd20c8497285"
    );
    let cert_count = std::str::from_utf8(&pem)
        .unwrap()
        .matches("-----BEGIN CERTIFICATE-----")
        .count();
    assert_eq!(cert_count, 118);

    let prov: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("boundary/trust/public-roots-v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(prov["sha256"], digest);
    assert_eq!(prov["size_bytes"], 200_116);
    assert_eq!(prov["certificate_count"], 118);
    assert_eq!(
        prov["image_path"],
        "/etc/duskweave/public-roots-2026-11-05.pem"
    );
    assert_eq!(prov["license"], "MPL-2.0");

    let df = std::fs::read_to_string(root.join("boundary/Dockerfile.https-public")).unwrap();
    assert!(df.starts_with("FROM scratch\n"));
    assert!(df.contains("COPY --chmod=0555 dw-dns-capture /dw-dns-capture"));
    assert!(df.contains("COPY --chmod=0555 dw-dns-exporter /dw-dns-exporter"));
    assert!(df.contains(
        "COPY --chmod=a=rX cacert-2026-11-05.pem /etc/duskweave/public-roots-2026-11-05.pem"
    ));
}
