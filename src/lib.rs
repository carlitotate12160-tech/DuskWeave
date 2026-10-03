#![forbid(unsafe_code)]
//! DuskWeave M0A: durable mission registration and sourced history.

pub mod input;
pub mod mission;
pub mod planning;
pub mod planning_assessment;
pub mod planning_history;
pub mod planning_input;
pub mod postgres_mission;
mod postgres_planning_history;
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
