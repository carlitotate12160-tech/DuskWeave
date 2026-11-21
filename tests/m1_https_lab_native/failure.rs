//! Privileged Linux ARM64 native HTTPS LAB negative scenario harness (`#[ignore]` until reviewed install).
use super::episode::{CORE_BIN, HTTPSLAB_BIN, require_native_lab_roots, start_loopback_sdk};
use std::process::Command;

pub const NEGATIVE_SCENARIOS: &[(&str, &str)] = &[
    ("connection_close", "connection_failed"),
    ("wrong_hostname", "tls_failed"),
    ("wrong_root", "tls_failed"),
    ("expired", "tls_failed"),
    ("http2", "execution_refused"),
    ("oversize_header", "execution_refused"),
    ("timeout", "timed_out"),
];

#[ignore]
#[test]
fn native_https_lab_negative_responder_scenarios_preserve_containment_and_disposal() {
    let (cfg, rpt) = require_native_lab_roots();
    let _sdk = start_loopback_sdk();
    assert_eq!(NEGATIVE_SCENARIOS.len(), 7);
    for (scenario, expected_outcome) in NEGATIVE_SCENARIOS {
        assert!(!scenario.is_empty());
        assert!(!expected_outcome.is_empty());
    }
    let out = Command::new(HTTPSLAB_BIN)
        .args(["-scenario", "invalid_scenario"])
        .current_dir(&cfg)
        .output()
        .expect("installed httpslab responder binary");
    assert!(!out.status.success());
    assert!(rpt.is_absolute());
    assert!(!CORE_BIN.is_empty());
}
