//! Privileged Linux ARM64 native HTTPS LAB qualification harness (`#[ignore]` until reviewed install).

#[path = "support/m1_https_lab.rs"]
mod lab_support;
#[path = "support/m1_genai_fixture.rs"]
mod sdk_fixture;

#[path = "m1_https_lab_native/active_effect.rs"]
mod active_effect;
#[path = "m1_https_lab_native/active_effect_evidence.rs"]
mod active_effect_evidence;
#[path = "m1_https_lab_native/active_effect_probe.rs"]
mod active_effect_probe;
#[path = "m1_https_lab_native/active_effect_tests.rs"]
mod active_effect_tests;
#[path = "m1_https_lab_native/d2_hold.rs"]
mod d2_hold;
#[path = "m1_https_lab_native/episode.rs"]
mod episode;
#[path = "m1_https_lab_native/failure.rs"]
mod failure;
#[path = "m1_https_lab_native/support.rs"]
mod support;
#[path = "m1_https_lab_native/topology.rs"]
mod topology;
#[cfg(target_os = "linux")]
#[path = "m1_https_lab_native/topology_linux.rs"]
mod topology_linux;
#[path = "m1_https_lab_native/topology_tests.rs"]
mod topology_tests;
