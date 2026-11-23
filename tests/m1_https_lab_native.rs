//! Privileged Linux ARM64 native HTTPS LAB qualification harness (`#[ignore]` until reviewed install).

#[path = "support/m1_https_lab.rs"]
mod lab_support;
#[path = "support/m1_genai_fixture.rs"]
mod sdk_fixture;

#[path = "m1_https_lab_native/episode.rs"]
mod episode;
#[path = "m1_https_lab_native/failure.rs"]
mod failure;
#[path = "m1_https_lab_native/support.rs"]
mod support;
