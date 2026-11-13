//! Closed deployment profiles. These immutable values grant no campaign authority.
use crate::{Fail, Res};

pub const RUN_PROFILE: &str = "autonomous-dns-fixture-v2";
pub const TIMING_PROFILE: &str = "reasoning-bounded-v1";
pub const LIVE_RUN_PROFILE: &str = "autonomous-dns-genai-live-v1";
pub const LAB_RUN_PROFILE: &str = "autonomous-dns-genai-lab-v1";

/// Version identities belong to the closed profile, independent of CLI composition.
pub struct SdkVersions {
    pub permission: &'static str,
    pub run: &'static str,
    pub pack: &'static str,
    pub qualification_purpose: Option<&'static str>,
}
pub fn sdk_versions(run_profile: &str) -> Res<SdkVersions> {
    let (permission, run, pack, qualification_purpose) = match run_profile {
        RUN_PROFILE => (
            "m1_inference_permission_v5",
            "m1_autonomous_run_request_v3",
            "m1_context_pack_autonomous_v3",
            None,
        ),
        LIVE_RUN_PROFILE => (
            "m1_inference_permission_v6",
            "m1_autonomous_run_request_v4",
            "m1_context_pack_autonomous_v4",
            Some("LIVE"),
        ),
        LAB_RUN_PROFILE => (
            "m1_inference_permission_v6",
            "m1_autonomous_run_request_v4",
            "m1_context_pack_autonomous_v4",
            Some("LAB"),
        ),
        _ => return Err(Fail::Input("inference_unsupported_profile")),
    };
    Ok(SdkVersions {
        permission,
        run,
        pack,
        qualification_purpose,
    })
}

pub fn resolver_binding(run_profile: &str, resolver: &str) -> bool {
    let Ok(ip) = resolver.parse::<std::net::Ipv4Addr>() else {
        return false;
    };
    if ip.to_string() != resolver {
        return false;
    }
    match run_profile {
        LAB_RUN_PROFILE => resolver == "192.0.2.53",
        LIVE_RUN_PROFILE => global_resolver(ip),
        _ => false,
    }
}

/// Closed semantic mapping only. Host qualification and current consent remain mandatory.
pub fn live_binding(run: &str, mode: &str, inference: &str) -> bool {
    mode == "live"
        && matches!(
            (run, inference),
            (
                LIVE_RUN_PROFILE,
                "deepseek-genai-responses-low-v1" | "deepseek-genai-v4-pro-low-v1"
            ) | (LAB_RUN_PROFILE, "fixture-genai-responses-v1")
        )
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Legacy,
    Genai,
}

#[derive(Clone, Copy)]
pub struct Profile {
    pub backend: Backend,
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

type Metadata = (
    &'static str,
    &'static str,
    Option<&'static str>,
    Protocol,
    &'static str,
    Option<&'static str>,
);
// Legacy deployment metadata is frozen separately from explicit SDK entries.
fn legacy_metadata(name: &str) -> Res<Metadata> {
    Ok(match name {
        "deepseek-responses-reasoning-v1" => (
            "deepseek-responses-reasoning-v1",
            "https://api.deepseek.com/responses",
            Some("DEEPSEEK_API_KEY"),
            Protocol::Responses,
            "deepseek-flash",
            Some("high"),
        ),
        "deepseek-responses-low-v1" => (
            "deepseek-responses-low-v1",
            "https://api.deepseek.com/responses",
            Some("DEEPSEEK_API_KEY"),
            Protocol::Responses,
            "deepseek-flash",
            Some("low"),
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
    })
}
pub fn lookup(name: &str) -> Res<Profile> {
    let (backend, metadata) = match name {
        "deepseek-genai-responses-low-v1" => (
            Backend::Genai,
            (
                "deepseek-genai-responses-low-v1",
                "https://api.deepseek.com/responses",
                Some("DEEPSEEK_API_KEY"),
                Protocol::Responses,
                "deepseek-flash",
                Some("low"),
            ),
        ),
        "deepseek-genai-v4-pro-low-v1" => (
            Backend::Genai,
            (
                "deepseek-genai-v4-pro-low-v1",
                "https://api.deepseek.com/responses",
                Some("DEEPSEEK_API_KEY"),
                Protocol::Responses,
                "deepseek-v4-pro",
                Some("low"),
            ),
        ),
        "fixture-genai-responses-v1" => (
            Backend::Genai,
            (
                "fixture-genai-responses-v1",
                "http://127.0.0.1:18083/responses",
                None,
                Protocol::Responses,
                "deepseek-flash",
                Some("low"),
            ),
        ),
        _ => (Backend::Legacy, legacy_metadata(name)?),
    };
    let (name, recipient, credential, protocol, model, effort) = metadata;
    if !REASONING_BOUNDED.feasible() {
        return Err(Fail::Config("inference_infeasible_profile"));
    }
    Ok(Profile {
        backend,
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

// Closed special-use policy for the selected public-resolver lane. LAB is separate.
fn global_resolver(ip: std::net::Ipv4Addr) -> bool {
    let n = u32::from(ip);
    const SPECIAL: &[(u32, u32)] = &[
        (0x00000000, 0xff000000),
        (0x0a000000, 0xff000000),
        (0x64400000, 0xffc00000),
        (0x7f000000, 0xff000000),
        (0xa9fe0000, 0xffff0000),
        (0xac100000, 0xfff00000),
        (0xc0000000, 0xffffff00),
        (0xc0000200, 0xffffff00),
        (0xc0586300, 0xffffff00),
        (0xc0a80000, 0xffff0000),
        (0xc6120000, 0xfffe0000),
        (0xc6336400, 0xffffff00),
        (0xcb007100, 0xffffff00),
        (0xe0000000, 0xe0000000),
    ];
    !SPECIAL.iter().any(|(network, mask)| n & mask == *network)
}
