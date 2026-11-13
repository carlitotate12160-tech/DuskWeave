//! Schema-0030 restricted-role SQL regression target.
//!
//! The normal CI wrapper bootstraps a dedicated fixture database through
//! m1_native_bootstrap.py (container `dw-live-dns-20261030-pg`, schema
//! revision 0031; retained 0030 contracts plus admission diagnostics), then re-invokes this same binary's worker assertions
//! under the fixture environment. Worker tests carry `#[ignore]` so they
//! are not discovered independently; the wrapper executes every case on
//! every CI run. Non-zero case counts and every exit/profile are propagated.
#![allow(dead_code)]

#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_genai_fixture.rs"]
mod genai;
#[path = "support/m1_terrain.rs"]
mod terrain_support;

#[path = "m1_live_dns_sql/admission.rs"]
mod admission;
#[path = "m1_live_dns_sql/audit007.rs"]
mod audit007;
#[path = "m1_live_dns_sql/audit007_support.rs"]
mod audit007_support;
#[path = "m1_live_dns_sql/diagnostic_episode.rs"]
mod diagnostic_episode;
#[path = "m1_live_dns_sql/fixture.rs"]
mod fixture;
#[path = "m1_live_dns_sql/lease_coordination.rs"]
mod lease_coordination;
#[path = "m1_live_dns_sql/lease_coordination_support.rs"]
mod lease_coordination_support;
#[path = "m1_live_dns_sql/pro_profile.rs"]
mod pro_profile;
#[path = "m1_live_dns_sql/receipt.rs"]
mod receipt;
#[path = "m1_live_dns_sql/run_support.rs"]
mod run_support;
#[path = "m1_live_dns_sql/successor_episode.rs"]
mod successor_episode;
#[path = "m1_live_dns_sql/successor_pack.rs"]
mod successor_pack;
#[path = "m1_live_dns_sql/terrain.rs"]
mod terrain;
#[path = "m1_live_dns_sql/withdrawal.rs"]
mod withdrawal;

#[test]
fn dedicated_schema0030_fixture_executes_the_sql_assertion_group() {
    fixture::run_fixture_wrapper();
}
