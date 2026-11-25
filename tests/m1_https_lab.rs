//! Closed HTTPS LAB protocol, effect-controller/launcher, and qualification unit contracts.
#[path = "support/m1_https_lab.rs"]
mod lab_support;

#[path = "m1_https_lab/launcher.rs"]
mod launcher;
#[path = "m1_https_lab/protocol.rs"]
mod protocol;
#[path = "m1_https_lab/qualification.rs"]
mod qualification;
