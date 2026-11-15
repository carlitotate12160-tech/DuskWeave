use super::*;
use duskweave::m1_inference_profile::LIVE_RUN_PROFILE;
use std::process::Command;

#[test]
fn rendered_live_candidate_manifest_round_trips_into_launcher_parser() {
    let dir = tempfile_dir("render-ok");
    let out = dir.join("deployment.json");
    let root = env!("CARGO_MANIFEST_DIR");
    let h = "a".repeat(64);
    let output = Command::new("bash")
        .arg(format!("{root}/boundary/qualify/qualify.sh"))
        .args([
            "candidate",
            "render-binding",
            "--release-sha256",
            &h,
            "--launcher-sha256",
            &h,
            "--capture-sha256",
            &h,
            "--exporter-sha256",
            &h,
            "--image-digest",
            &format!("sha256:{h}"),
            "--boot-id",
            "11111111-1111-4111-8111-111111111111",
            "--core-uid",
            "1001",
            "--resolver-ipv4",
            "1.1.1.1",
            "--valid-from-ms",
            "1000",
            "--valid-until-ms",
            "4000000000000",
            "--output-file",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("spawn qualify.sh candidate render-binding");
    assert!(
        output.status.success(),
        "render-binding failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse render evidence JSON");
    assert_eq!(evidence["status"], "UNAPPLIED");
    assert_eq!(evidence["applied"], false);

    let bytes = std::fs::read(&out).expect("read rendered manifest bytes");
    assert!(bytes.len() <= 16384);
    let qualified =
        parse_manifest(&bytes).expect("launcher parse_manifest must accept rendered bytes");
    qualified
        .manifest
        .validate()
        .expect("launcher Manifest::validate must accept LIVE manifest");
    assert_eq!(
        qualified.manifest_sha256,
        evidence["manifest_sha256"].as_str().unwrap()
    );

    let binding = LiveBinding {
        qualification_ref: qualified.manifest.qualification_ref,
        release_sha256: h,
        resolver_ipv4: "1.1.1.1".into(),
    };
    assert!(qualified.manifest.matches(&binding, LIVE_RUN_PROFILE));

    let oversized = vec![b' '; 16385];
    assert!(parse_manifest(&oversized).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn render_binding_refuses_lab_resolver_6to4_and_lab_profile_and_accepts_neighbors() {
    let root = env!("CARGO_MANIFEST_DIR");
    let h = "b".repeat(64);
    for (flag, value) in [
        ("--resolver-ipv4", "192.0.2.53"),
        ("--resolver-ipv4", "192.88.99.1"),
        ("--run-profile", "autonomous-dns-genai-lab-v1"),
    ] {
        let mut cmd = Command::new("bash");
        cmd.arg(format!("{root}/boundary/qualify/qualify.sh"))
            .args([
                "candidate",
                "render-binding",
                "--release-sha256",
                &h,
                "--launcher-sha256",
                &h,
                "--capture-sha256",
                &h,
                "--exporter-sha256",
                &h,
                "--image-digest",
                &format!("sha256:{h}"),
                "--boot-id",
                "11111111-1111-4111-8111-111111111111",
                "--core-uid",
                "1001",
                "--valid-from-ms",
                "1000",
                "--valid-until-ms",
                "4000000000000",
            ]);
        if flag == "--resolver-ipv4" {
            cmd.args([flag, value]);
        } else {
            cmd.args(["--resolver-ipv4", "1.1.1.1", flag, value]);
        }
        let output = cmd.output().expect("spawn qualify.sh refusal check");
        assert!(
            !output.status.success(),
            "expected refusal for {flag}={value}"
        );
    }

    for (ip, expected_ok) in [
        ("192.88.99.1", false),
        ("192.88.98.255", true),
        ("192.88.100.0", true),
    ] {
        let b = LiveBinding {
            qualification_ref: Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap(),
            release_sha256: h.clone(),
            resolver_ipv4: ip.into(),
        };
        assert_eq!(b.validate_for(LIVE_RUN_PROFILE).is_ok(), expected_ok, "rust validate {ip}");
        if expected_ok {
            let out = Command::new("bash")
                .arg(format!("{root}/boundary/qualify/qualify.sh"))
                .args([
                    "candidate", "render-binding",
                    "--release-sha256", &h, "--launcher-sha256", &h,
                    "--capture-sha256", &h, "--exporter-sha256", &h,
                    "--image-digest", &format!("sha256:{h}"),
                    "--boot-id", "11111111-1111-4111-8111-111111111111",
                    "--core-uid", "1001", "--resolver-ipv4", ip,
                    "--valid-from-ms", "1000", "--valid-until-ms", "4000000000000",
                ])
                .output()
                .expect("render neighbor ip");
            assert!(out.status.success(), "python render failed for {ip}");
        }
    }
}

fn tempfile_dir(prefix: &str) -> std::path::PathBuf {
    let id = std::fs::read_to_string("/proc/sys/kernel/random/uuid").unwrap();
    let path = std::env::temp_dir().join(format!("dw-cand-bind-{prefix}-{}", id.trim()));
    std::fs::create_dir_all(&path).unwrap();
    path
}
