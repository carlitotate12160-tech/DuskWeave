#![forbid(unsafe_code)]
//! DuskWeave M0A: durable mission registration and sourced history.

pub mod input;
pub mod m1_inference_budget;
pub mod m1_inference_permission;
pub mod m1_inference_profile;
pub mod m1_permission;
pub mod m1_policy;
pub mod m1_policy_input;
pub mod m1_prepared_withdrawal;
pub mod m1_reasoning;
pub mod m1_session;
pub mod m1_session_input;
pub mod mission;
pub mod planning;
pub mod planning_assessment;
pub mod planning_history;
pub mod planning_input;
pub mod postgres_inference_budget;
pub mod postgres_m1_session;
pub mod postgres_mission;
mod postgres_planning_history;
pub mod postgres_prepared_withdrawal;
pub mod postgres_trajectory;
mod postgres_trajectory_history;
mod postgres_trajectory_journal;
pub mod registration;
pub mod trajectory;
pub mod withdrawal;
pub mod withdrawal_input;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fail {
    Input(&'static str),
    Conflict(&'static str),
    State(&'static str),
    Unresolved(&'static str),
    Store(&'static str),
    Config(&'static str),
}

pub type Res<T> = Result<T, Fail>;

pub mod m1_context;
pub mod m1_context_capacity;
pub mod m1_context_input;
pub mod m1_terrain;
pub mod m1_terrain_input;
pub mod m1_terrain_view;
pub mod postgres_m1_terrain;
pub mod postgres_terrain_history;
pub mod postgres_terrain_view;

pub mod m1_autonomous_context;
pub mod m1_boundary_dns;
pub mod m1_fixture_dns;
pub mod m1_run;
pub mod m1_terrain_issued;
pub mod postgres_m1_run;
