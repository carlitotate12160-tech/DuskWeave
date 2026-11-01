//! Root-installed release identity. An ordinary caller cannot mint qualification.
use duskweave::m1_inference_permission::LiveBinding;
use duskweave::{Fail, Res};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
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
    let valid = [m.is_dir(), m.uid() == 0, m.mode() & 0o022 == 0];
    valid
        .into_iter()
        .all(|v| v)
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
    let valid = [
        m.is_file(),
        m.uid() == 0,
        m.nlink() == 1,
        m.mode() & 0o022 == 0,
        m.len() <= 134217728,
    ];
    valid
        .into_iter()
        .all(|v| v)
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

fn read_manifest() -> Res<Qualified> {
    let mut bytes = Vec::new();
    root_file(CONFIG)?
        .take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| Fail::Config("live_host_unqualified"))?;
    if bytes.len() > 16384 {
        return Err(Fail::Config("live_host_unqualified"));
    }
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|_| Fail::Config("live_host_unqualified"))?;
    Ok(Qualified {
        manifest,
        manifest_sha256: Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    })
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
            if digest(root_file(&format!("{BIN_ROOT}/{name}"))?)? != *expected {
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
