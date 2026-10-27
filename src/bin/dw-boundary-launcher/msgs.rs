//! Message types for the ADR-017 control channel and the attempt request.
use serde::{Deserialize, Serialize};

pub const REQUEST_VERSION: &str = "dw_boundary_request_v1";
pub const RESULT_VERSION: &str = "dw_boundary_result_v1";
pub const ATTEMPT_VERSION: &str = "dw_boundary_attempt_v1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    pub ipv4: String,
    pub port: u16,
    pub transport: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptRequest {
    pub version: String,
    pub session_id: String,
    pub attempt_id: String,
    pub control_root: String,
    pub image: String,
    pub effect: String,
    pub qname: String,
    pub qtype: String,
    pub endpoint: Endpoint,
    pub txid: u16,
    pub deadline_ms: i64,
    pub max_wire: u32,
    pub max_raw: u32,
    pub lease_ms: u64,
    pub renew_ms: u64,
}

/// Owned copy of the issued fields the drive thread needs; the pipes carry
/// these values, never credentials or arbitrary operator text.
pub struct Owned {
    pub attempt_id: String,
    pub session_id: String,
    pub effect: String,
    pub qname: String,
    pub qtype: String,
    pub endpoint_ipv4: String,
    pub endpoint_port: u16,
    pub endpoint_transport: String,
    pub txid: u16,
    pub deadline_ms: i64,
    pub max_wire: u32,
    pub max_raw: u32,
    pub lease_ms: u64,
}

#[derive(Serialize)]
pub struct CaptureRequest<'a> {
    pub version: &'a str,
    pub kind: &'a str,
    pub attempt_id: &'a str,
    pub session_id: &'a str,
    pub effect: &'a str,
    pub qname: &'a str,
    pub qtype: &'a str,
    pub endpoint: EndpointRef<'a>,
    pub txid: u16,
    pub deadline_ms: i64,
    pub max_wire: u32,
}

#[derive(Serialize)]
pub struct EndpointRef<'a> {
    pub ipv4: &'a str,
    pub port: u16,
    pub transport: &'a str,
}

#[derive(Serialize)]
pub struct ExporterRequest<'a> {
    pub version: &'a str,
    pub kind: &'a str,
    pub attempt_id: &'a str,
    pub session_id: &'a str,
    pub effect: &'a str,
    pub qname: &'a str,
    pub qtype: &'a str,
    pub txid: u16,
    pub deadline_ms: i64,
    pub max_raw: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phase {
    pub version: String,
    pub phase: String,
    #[serde(default)]
    pub kind: String,
    pub attempt_id: String,
}

#[derive(Serialize)]
pub struct Control<'a> {
    pub version: &'a str,
    pub phase: &'a str,
    pub attempt_id: &'a str,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    pub ipv4: String,
    pub ttl: u32,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Counters {
    pub raw_bytes: u32,
    pub raw_truncated: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResult {
    pub version: String,
    pub phase: String,
    pub attempt_id: String,
    pub disposition: String,
    pub rcode: String,
    pub truncated: bool,
    pub answers: Vec<Answer>,
    pub counters: Counters,
    pub elapsed_ms: i64,
}
