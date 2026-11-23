#![allow(clippy::too_many_arguments)]

#[path = "support/registration_db.rs"]
pub mod db_support;
#[path = "support/m1_genai_fixture.rs"]
pub mod genai;
#[path = "support/m1_https_lab.rs"]
pub mod lab_support;
#[path = "support/m1_terrain_context.rs"]
pub mod terrain;

#[path = "m1_https_lab_postgres/admission.rs"]
mod admission;
#[path = "m1_https_lab_postgres/denied_release.rs"]
mod denied_release;
#[path = "m1_https_lab_postgres/episode.rs"]
mod episode;
#[path = "m1_https_lab_postgres/lineage.rs"]
mod lineage;
#[path = "m1_https_lab_postgres/recovery.rs"]
mod recovery;
#[path = "m1_https_lab_postgres/upgrade.rs"]
mod upgrade;
