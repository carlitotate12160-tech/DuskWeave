//! Closed deployment profile bindings and resolver/SDK version mappings.
use crate::{Fail, Res};
use std::net::Ipv4Addr;

pub const RUN_PROFILE: &str = "autonomous-dns-fixture-v2";
pub const TIMING_PROFILE: &str = "reasoning-bounded-v1";
pub const PRO_V2_TIMING_PROFILE: &str = "reasoning-bounded-pro-v2";
pub const PRO_V2_PROFILE: &str = "deepseek-genai-v4-pro-low-v2";
pub const FIXTURE_PRO_V2_PROFILE: &str = "fixture-genai-v4-pro-low-v2";
pub const HTTPS_LOOP_RUN_PROFILE: &str = "m1_https_loop_v1";
pub const LIVE_RUN_PROFILE: &str = "autonomous-dns-genai-live-v1";
pub const LAB_RUN_PROFILE: &str = "autonomous-dns-genai-lab-v1";
pub const HTTPS_LAB_RUN_PROFILE: &str = "autonomous-https-genai-lab-v1";
pub const HTTPS_LAB_PURPOSE: &str = "HTTPS_LOOP_LAB";
pub const HTTPS_LAB_HOSTNAME: &str = "head.duskweave.test";
pub const HTTPS_LAB_IPV4: &str = "198.51.100.10";
pub const HTTPS_LAB_RESOLVER: &str = "192.0.2.53";
pub const HTTPS_HEAD_PROFILE: &str = "https-head-v1";

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
        HTTPS_LOOP_RUN_PROFILE => (
            "m1_https_loop_v1",
            "m1_https_loop_v1",
            "m1_https_loop_v1",
            None,
        ),
        HTTPS_LAB_RUN_PROFILE => (
            "m1_https_loop_v1",
            "m1_https_loop_v1",
            "m1_https_loop_v1",
            Some(HTTPS_LAB_PURPOSE),
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
    let Ok(ip) = resolver.parse::<Ipv4Addr>() else {
        return false;
    };
    if ip.to_string() != resolver {
        return false;
    }
    match run_profile {
        LAB_RUN_PROFILE | HTTPS_LAB_RUN_PROFILE => resolver == HTTPS_LAB_RESOLVER,
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
                "deepseek-genai-responses-low-v1" | "deepseek-genai-v4-pro-low-v1" | PRO_V2_PROFILE
            ) | (LAB_RUN_PROFILE, "fixture-genai-responses-v1")
                | (HTTPS_LAB_RUN_PROFILE, FIXTURE_PRO_V2_PROFILE)
        )
}

// Closed special-use policy for the selected public-resolver lane. LAB is separate.
fn global_resolver(ip: Ipv4Addr) -> bool {
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
