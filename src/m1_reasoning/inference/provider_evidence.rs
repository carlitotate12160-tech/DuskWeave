//! Existing provider usage and terminal-envelope metadata; values stay transport-owned.
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub response_bytes: usize,
    pub wall_time_ms: u64,
    pub requested_model: &'static str,
    pub reported_model: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Completed,
    Incomplete,
    Failed,
    InProgress,
    Unrecognized,
    Missing,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IncompleteReason {
    MaxOutputTokens,
    ContentFilter,
    Unknown,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageAvailability {
    Reported,
    Partial,
    Unavailable,
}
#[derive(Clone, Debug, Serialize)]
pub struct ResponseMetadata {
    pub status: ResponseStatus,
    pub incomplete_reason: Option<IncompleteReason>,
    pub usage_availability: UsageAvailability,
}
#[derive(Clone, Debug, Serialize)]
pub struct ProviderReport {
    #[serde(flatten)]
    pub usage: Usage,
    pub provider_response: ResponseMetadata,
}
