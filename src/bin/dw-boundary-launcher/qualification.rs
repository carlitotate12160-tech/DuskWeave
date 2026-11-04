//! Root-installed release identity. An ordinary caller cannot mint qualification.
use duskweave::m1_inference_permission::LiveBinding;
use duskweave::{Fail, Res};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use uuid::Uuid;

const CONFIG: &str = "/etc/duskweave/m1/deployment.json";
const BIN_ROOT: &str = "/usr/local/libexec/duskweave";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    pub qualification_ref: Uuid,
    pub purpose: String,
    pub run_profile: String,
    pub release_sha256: String,
    pub launcher_sha256: String,
    pub capture_sha256: String,
    pub exporter_sha256: String,
    pub image_digest: String,
    pub boot_id: Uuid,
    pub resolver_ipv4: String,
    pub valid_from_ms: i64,
    pub valid_until_ms: i64,
    pub core_uid: u32,
}

#[cfg(test)]
thread_local! {
    static TEST_CONFIG: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
    static TEST_BIN_ROOT: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(super) fn with_test_env<F, R>(
    config: std::path::PathBuf,
    bin_root: std::path::PathBuf,
    f: F,
) -> R
where
    F: FnOnce() -> R,
{
    struct Restore {
        config: Option<std::path::PathBuf>,
        bin_root: Option<std::path::PathBuf>,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_CONFIG.with(|c| *c.borrow_mut() = self.config.take());
            TEST_BIN_ROOT.with(|b| *b.borrow_mut() = self.bin_root.take());
        }
    }
    let restore = Restore {
        config: TEST_CONFIG.with(|c| c.borrow_mut().take()),
        bin_root: TEST_BIN_ROOT.with(|b| b.borrow_mut().take()),
    };
    TEST_CONFIG.with(|c| *c.borrow_mut() = Some(config));
    TEST_BIN_ROOT.with(|b| *b.borrow_mut() = Some(bin_root));
    let res = f();
    drop(restore);
    res
}

pub(super) struct Qualified {
    pub manifest: Manifest,
    pub manifest_sha256: String,
}

pub(super) fn now_ms() -> Res<i64> {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Fail::Config("live_host_unqualified"))?
            .as_millis(),
    )
    .map_err(|_| Fail::Config("live_host_unqualified"))
}

fn root_directory(path: &str) -> Res<()> {
    let m = std::fs::symlink_metadata(path).map_err(|_| Fail::Config("live_host_unqualified"))?;
    let f = super::custody::observed(&m);
    super::custody::deploy_directory(&f)
        .then_some(())
        .ok_or(Fail::Config("live_host_unqualified"))
}

fn root_file(path: &str) -> Res<File> {
    use rustix::fs::{CWD, Mode, OFlags, ResolveFlags, openat2};
    let fd = openat2(
        CWD,
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS,
    )
    .map_err(|_| Fail::Config("live_host_unqualified"))?;
    let file = File::from(fd);
    qualify_file(&file)?;
    Ok(file)
}

fn qualify_file(file: &File) -> Res<()> {
    let m = file
        .metadata()
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    let f = super::custody::observed(&m);
    super::custody::deploy_file(&f)
        .then_some(())
        .ok_or(Fail::Config("live_host_unqualified"))
}

fn digest(mut file: File) -> Res<String> {
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| Fail::Config("live_host_unqualified"))?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn executable_digest(path: &str) -> Res<String> {
    // Only the two kernel-owned /proc executable links use normal descriptor opens.
    let file = File::open(path).map_err(|_| Fail::Config("live_host_unqualified"))?;
    qualify_file(&file)?;
    digest(file)
}

impl Manifest {
    fn validate(&self) -> Res<()> {
        let binding = LiveBinding {
            qualification_ref: self.qualification_ref,
            release_sha256: self.release_sha256.clone(),
            resolver_ipv4: self.resolver_ipv4.clone(),
        };
        binding.validate_for(&self.run_profile)?;
        let versions = duskweave::m1_inference_profile::sdk_versions(&self.run_profile)?;
        let n = now_ms()?;
        let valid = [
            versions.qualification_purpose == Some(self.purpose.as_str()),
            !self.boot_id.is_nil(),
            self.core_uid != 0,
            self.valid_from_ms <= n,
            n < self.valid_until_ms,
            [
                self.launcher_sha256.as_str(),
                self.capture_sha256.as_str(),
                self.exporter_sha256.as_str(),
            ]
            .into_iter()
            .all(hex),
            self.image_digest.strip_prefix("sha256:").is_some_and(hex),
        ];
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or(Fail::Config("live_host_unqualified"))
    }
    pub fn matches(&self, binding: &LiveBinding, profile: &str) -> bool {
        [
            self.qualification_ref == binding.qualification_ref,
            self.release_sha256 == binding.release_sha256,
            self.resolver_ipv4 == binding.resolver_ipv4,
            self.run_profile == profile,
        ]
        .into_iter()
        .all(|v| v)
    }
}

fn hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn host_sinks() -> Res<()> {
    let swap = std::fs::read_to_string("/proc/swaps")
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    let pattern = std::fs::read_to_string("/proc/sys/kernel/core_pattern")
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    let suid = std::fs::read_to_string("/proc/sys/fs/suid_dumpable")
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    [
        swap.lines().count() == 1,
        pattern.trim() == "|/bin/false",
        suid.trim() == "0",
    ]
    .into_iter()
    .all(|v| v)
    .then_some(())
    .ok_or(Fail::Config("live_host_unqualified"))
}

fn protected_directories() -> Res<()> {
    for directory in [
        "/etc",
        "/etc/duskweave",
        "/etc/duskweave/m1",
        "/usr/local",
        "/usr/local/libexec",
        BIN_ROOT,
    ] {
        root_directory(directory)?;
    }
    Ok(())
}

pub(super) fn parse_manifest(bytes: &[u8]) -> Res<Qualified> {
    if bytes.len() > 16384 {
        return Err(Fail::Config("live_host_unqualified"));
    }
    let manifest: Manifest =
        serde_json::from_slice(bytes).map_err(|_| Fail::Config("live_host_unqualified"))?;
    Ok(Qualified {
        manifest,
        manifest_sha256: Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    })
}

fn config_path() -> String {
    #[cfg(test)]
    if let Some(p) = TEST_CONFIG.with(|c| {
        c.borrow()
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
    }) {
        return p;
    }
    CONFIG.to_string()
}

fn bin_path(name: &str) -> String {
    #[cfg(test)]
    if let Some(root) = TEST_BIN_ROOT.with(|b| {
        b.borrow()
            .as_ref()
            .map(|p| p.join(name).to_string_lossy().into_owned())
    }) {
        return root;
    }
    format!("{BIN_ROOT}/{name}")
}

fn read_manifest() -> Res<Qualified> {
    let mut bytes = Vec::new();
    root_file(&config_path())?
        .take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    parse_manifest(&bytes)
}

impl Manifest {
    fn verify_process(&self, peer: rustix::net::UCred) -> Res<()> {
        let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .map_err(|_| Fail::Config("live_host_unqualified"))?;
        let peer_path = format!("/proc/{}/exe", peer.pid.as_raw_nonzero());
        let valid = [
            peer.uid.as_raw() == self.core_uid,
            boot.trim() == self.boot_id.to_string(),
            executable_digest(&peer_path)? == self.release_sha256,
            executable_digest("/proc/self/exe")? == self.launcher_sha256,
        ];
        if !valid.into_iter().all(|v| v) {
            return Err(Fail::Config("live_host_unqualified"));
        }
        Ok(())
    }
    fn verify_image(&self) -> Res<()> {
        for (name, expected) in [
            ("dw-dns-capture", &self.capture_sha256),
            ("dw-dns-exporter", &self.exporter_sha256),
        ] {
            if digest(root_file(&bin_path(name))?)? != *expected {
                return Err(Fail::Config("live_host_unqualified"));
            }
        }
        super::docker::qualify(&self.image_digest)
    }
}

pub(super) fn identity(peer: rustix::net::UCred) -> Res<Qualified> {
    protected_directories()?;
    let qualified = read_manifest()?;
    qualified.manifest.verify_process(peer)?;
    Ok(qualified)
}
pub(super) fn acquisition(qualified: &Qualified) -> Res<()> {
    qualified.manifest.validate()?;
    qualified.manifest.verify_image()?;
    host_sinks()
}

/// Only the installed root launcher may create a stop-only watchdog peer.
pub(super) fn watchdog_peer(peer: rustix::net::UCred) -> Res<()> {
    protected_directories()?;
    let qualified = read_manifest()?;
    let expected = &qualified.manifest.launcher_sha256;
    let peer_path = format!("/proc/{}/exe", peer.pid.as_raw_nonzero());
    let valid = [
        peer.uid.as_raw() == 0,
        executable_digest(&peer_path)? == *expected,
        executable_digest("/proc/self/exe")? == *expected,
    ];
    valid
        .into_iter()
        .all(|v| v)
        .then_some(())
        .ok_or(Fail::Config("live_host_unqualified"))
}

#[cfg(test)]
#[path = "../../../tests/m1_live_dns/units/launcher_qualification.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/m1_live_dns/units/qualification_custody.rs"]
mod custody_tests;
