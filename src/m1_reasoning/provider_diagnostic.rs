use crate::m1_reasoning::bounded_json::JsonFailure;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    PreSpawn,
    Transport,
    Http,
    Json,
    Envelope,
    Completion,
}
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    #[default]
    Refused,
    TransportFailure,
    UnsafeResponse,
    ResponseLimit,
    InvalidHeaders,
    UpgradeRefused,
    UnsupportedEncoding,
    HttpError,
    InvalidJson,
    JsonBoundExceeded,
    JsonStructureRefused,
    EnvelopeRejected,
    Completed,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    AuthenticationError,
    InvalidRequestError,
    RateLimitError,
    InsufficientQuota,
    ServerError,
    Unknown,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorParam {
    Model,
    Input,
    Instructions,
    Stream,
    Tools,
    ToolChoice,
    MaxOutputTokens,
    Reasoning,
    #[serde(rename = "reasoning.effort")]
    ReasoningEffort,
    Text,
    #[serde(rename = "text.format")]
    TextFormat,
    Unknown,
}
#[derive(Debug, Default, Serialize)]
pub struct ProviderDiagnostic {
    pub binding_digest: String,
    pub phase: Phase,
    pub classification: Classification,
    pub final_http_status: Option<u16>,
    pub curl_exit_code: Option<i32>,
    pub response_bytes: usize,
    pub interim_responses: u16,
    pub error_type: Option<ErrorType>,
    pub error_param: Option<ErrorParam>,
    pub json_error: Option<JsonFailure>,
    pub report: Option<super::inference::ProviderReport>,
}
impl ProviderDiagnostic {
    pub fn new(binding_digest: String) -> Self {
        Self {
            binding_digest,
            ..Default::default()
        }
    }
}
