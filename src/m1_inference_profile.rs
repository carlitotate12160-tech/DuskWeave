//! Closed deployment profiles. These immutable values grant no campaign authority.
use crate::{Fail, Res};

pub const RUN_PROFILE: &str = "autonomous-dns-fixture-v2";
pub const TIMING_PROFILE: &str = "reasoning-bounded-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Responses,
    ChatCompletions,
}

#[derive(Clone, Copy)]
pub struct Connection {
    pub recipient: &'static str,
    pub credential_environment: Option<&'static str>,
}

#[derive(Clone, Copy)]
pub struct ModelSettings {
    pub model: &'static str,
    pub reasoning_effort: Option<&'static str>,
    pub output_tokens: u32,
    pub stream: bool,
}

#[derive(Clone, Copy)]
pub struct RoutingPolicy {
    pub redirects: bool,
    pub retries: bool,
    pub proxy: bool,
}

#[derive(Clone, Copy)]
pub struct Profile {
    pub name: &'static str,
    pub connection: Connection,
    pub protocol: Protocol,
    pub settings: ModelSettings,
    pub routing: RoutingPolicy,
    pub timing: &'static Timing,
}

/// Resource-policy ceilings; these are not calibrated provider latency.
pub struct Timing {
    pub connect_ms: u64,
    pub soft_ms: u64,
    pub hard_ms: u64,
    pub shared_ms: u64,
    pub decision_ms: u64,
    pub phase_ms: u64,
    pub child_watchdog_ms: u64,
    pub acquisition_ms: u64,
    pub run_ms: u64,
    pub parent_watchdog_ms: u64,
    pub stop_ms: u64,
    pub accounting_ms: u64,
}
pub const REASONING_BOUNDED: Timing = Timing {
    connect_ms: 5000,
    soft_ms: 60000,
    hard_ms: 62000,
    shared_ms: 124000,
    decision_ms: 129000,
    phase_ms: 134000,
    child_watchdog_ms: 139000,
    acquisition_ms: 60000,
    run_ms: 400000,
    parent_watchdog_ms: 410000,
    stop_ms: 2000,
    accounting_ms: 3000,
};

impl Timing {
    pub fn feasible(&self) -> bool {
        let valid = [
            self.connect_ms <= self.soft_ms,
            self.soft_ms.checked_add(self.stop_ms) == Some(self.hard_ms),
            self.hard_ms.checked_mul(2) == Some(self.shared_ms),
            self.shared_ms
                .checked_add(self.stop_ms)
                .and_then(|n| n.checked_add(self.accounting_ms))
                == Some(self.decision_ms),
            self.decision_ms.checked_add(5000) == Some(self.phase_ms),
            self.phase_ms < self.child_watchdog_ms,
            self.phase_ms
                .checked_mul(2)
                .and_then(|n| n.checked_add(self.acquisition_ms))
                .is_some_and(|n| n <= self.run_ms),
            self.run_ms < self.parent_watchdog_ms,
        ];
        valid.into_iter().all(|v| v)
    }
}

pub fn lookup(name: &str) -> Res<Profile> {
    let (name, recipient, credential, protocol, model, effort) = match name {
        "deepseek-responses-reasoning-v1" => (
            "deepseek-responses-reasoning-v1",
            "https://api.deepseek.com/responses",
            Some("DEEPSEEK_API_KEY"),
            Protocol::Responses,
            "deepseek-flash",
            Some("high"),
        ),
        "fixture-responses-v1" => (
            "fixture-responses-v1",
            "urn:duskweave:fixture:responses:v1",
            None,
            Protocol::Responses,
            "protocol-fixture-v1",
            None,
        ),
        "fixture-chat-completions-v1" => (
            "fixture-chat-completions-v1",
            "urn:duskweave:fixture:chat-completions:v1",
            None,
            Protocol::ChatCompletions,
            "protocol-fixture-v1",
            None,
        ),
        _ => return Err(Fail::Input("inference_unsupported_profile")),
    };
    if !REASONING_BOUNDED.feasible() {
        return Err(Fail::Config("inference_infeasible_profile"));
    }
    Ok(Profile {
        name,
        connection: Connection {
            recipient,
            credential_environment: credential,
        },
        protocol,
        settings: ModelSettings {
            model,
            reasoning_effort: effort,
            output_tokens: 4096,
            stream: false,
        },
        routing: RoutingPolicy {
            redirects: false,
            retries: false,
            proxy: false,
        },
        timing: &REASONING_BOUNDED,
    })
}
