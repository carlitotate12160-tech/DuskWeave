//! Closed HTTPS LAB qualification manifest parsing and root CA custody verification.
use duskweave::{Fail, Res};
use serde::Deserialize;
use uuid::Uuid;

pub(in super::super) const HTTPS_LAB_ROOT_VERSION: &str = "duskweave-https-lab-root-v1";
pub(in super::super) const HTTPS_LAB_ROOT_SHA256: &str =
    "af8714b55edd3a0a47cf24a1b24a6e99d9026ed4dd074b5a972a60618103d1dc";
const ROOT_PEM: &str = "/etc/duskweave/https-lab-root-v1.pem";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HttpsLabManifest {
    qualification_ref: Uuid,
    purpose: String,
    run_profile: String,
    release_sha256: String,
    launcher_sha256: String,
    capture_sha256: String,
    exporter_sha256: String,
    image_digest: String,
    boot_id: Uuid,
    resolver_ipv4: String,
    https_lab_root_version: String,
    https_lab_root_sha256: String,
    valid_from_ms: i64,
    valid_until_ms: i64,
    core_uid: u32,
}

#[cfg(test)]
thread_local! {
    static TEST_ROOT_PEM: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(in super::super) fn with_test_root_pem<F, R>(path: std::path::PathBuf, f: F) -> R
where
    F: FnOnce() -> R,
{
    struct Restore(Option<std::path::PathBuf>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_ROOT_PEM.with(|p| *p.borrow_mut() = self.0.take());
        }
    }
    let prev = Restore(TEST_ROOT_PEM.with(|p| p.borrow_mut().take()));
    TEST_ROOT_PEM.with(|p| *p.borrow_mut() = Some(path));
    let out = f();
    drop(prev);
    out
}

fn root_pem_path() -> String {
    #[cfg(test)]
    if let Some(p) = TEST_ROOT_PEM.with(|p| {
        p.borrow()
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
    }) {
        return p;
    }
    ROOT_PEM.to_string()
}

pub(super) fn parse_manifest(bytes: &[u8]) -> Res<super::Manifest> {
    let raw: HttpsLabManifest =
        serde_json::from_slice(bytes).map_err(|_| Fail::Config("live_host_unqualified"))?;
    let valid = [
        raw.purpose == "HTTPS_LOOP_LAB",
        raw.run_profile == "autonomous-https-genai-lab-v1",
        raw.resolver_ipv4 == "192.0.2.53",
        raw.https_lab_root_version == HTTPS_LAB_ROOT_VERSION,
        raw.https_lab_root_sha256 == HTTPS_LAB_ROOT_SHA256,
    ];
    if !valid.into_iter().all(|v| v) {
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(super::Manifest {
        qualification_ref: raw.qualification_ref,
        purpose: raw.purpose,
        run_profile: raw.run_profile,
        release_sha256: raw.release_sha256,
        launcher_sha256: raw.launcher_sha256,
        capture_sha256: raw.capture_sha256,
        exporter_sha256: raw.exporter_sha256,
        image_digest: raw.image_digest,
        boot_id: raw.boot_id,
        resolver_ipv4: raw.resolver_ipv4,
        valid_from_ms: raw.valid_from_ms,
        valid_until_ms: raw.valid_until_ms,
        core_uid: raw.core_uid,
    })
}

pub(super) fn verify_root_custody() -> Res<()> {
    let path = root_pem_path();
    let file = super::root_file(&path)?;
    if super::digest(file)? != HTTPS_LAB_ROOT_SHA256 {
        return Err(Fail::Config("live_host_unqualified"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../tests/m1_live_dns/units/launcher_https_qualification.rs"]
mod tests;
