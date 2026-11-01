//! Closed live DNS contracts. Port controls are separate from host qualification.
#[path = "m1_live_dns/admission.rs"]
mod admission;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "m1_live_dns/episode.rs"]
mod episode;
#[path = "m1_live_dns/fixture.rs"]
mod fixture;
#[path = "m1_live_dns/ipc.rs"]
mod ipc;
#[path = "support/m1_genai_fixture.rs"]
mod sdk_fixture;
#[path = "m1_live_dns/session.rs"]
mod session;
#[path = "m1_live_dns/terrain.rs"]
mod terrain;

#[path = "m1_live_dns/withdrawal.rs"]
mod withdrawal;

#[cfg(target_os = "linux")]
#[path = "m1_live_dns/host.rs"]
mod host;
#[cfg(target_os = "linux")]
#[path = "m1_live_dns/lab.rs"]
mod lab;

#[cfg(target_os = "linux")]
#[path = "m1_live_dns/recovery.rs"]
mod recovery;
