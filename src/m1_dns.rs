//! Closed capability request/result port. No process, persistence or network implementation.
mod codec;
mod request;
mod result;
use crate::Res;
pub use codec::{Control, FrameReader, FrameWriter};
pub use request::DnsRequest;
pub use result::{DnsAnswer, DnsResult, DnsStatus};

pub const SCHEMA: &str = "m1_dns_ipc_v1";
pub const PROFILE: &str = "dns-routing-v2";
pub const MAX_FRAME: usize = 16384;
pub const MAX_CONTROL: usize = 1024;

/// Only an independently qualified boundary implements this port.
pub trait DnsBoundary {
    fn ready(&mut self, request: &DnsRequest) -> Res<()>;
    fn start(&mut self, request: &DnsRequest, deadline_at_ms: i64) -> Res<()>;
    fn result(&mut self) -> Res<Option<DnsResult>>;
    fn renew(&mut self, request: &DnsRequest, deadline_at_ms: i64) -> Res<()>;
    fn acknowledge(&mut self, request: &DnsRequest) -> Res<bool>;
    fn stop(&mut self) -> Res<bool>;
}
