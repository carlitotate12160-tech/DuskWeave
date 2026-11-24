#![allow(dead_code)]
//! Shared config, responder provenance, scoped netns topology, and SQL/CLI helpers for native HTTPS LAB.

use crate::lab_support::{lab_proposal_for_pack, lab_registration_json, lab_run_request_json};
use crate::sdk_fixture::{Reply, Server, envelope};
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

pub(super) const CORE_BIN: &str = "/usr/local/libexec/duskweave/duskweave";
pub(super) const DEFAULT_BUILD_ROOT: &str = "/srv/duskweave-build/https-lab-r1";
pub(super) const OWNER_MARKER: &str = "/etc/duskweave/m1/.live-dns-owner";
pub(super) const DEPLOYMENT_MANIFEST: &str = "/etc/duskweave/m1/deployment.json";

pub(super) fn extract_https_context_pack(wire: &Value) -> Result<Value, &'static str> {
    let items = wire["input"]
        .as_array()
        .ok_or("missing_or_malformed_https_context_pack")?;
    for item in items {
        if item["role"].as_str() != Some("user") {
            continue;
        }
        let Some(raw) = item["content"].as_str() else {
            continue;
        };
        let Ok(pack) = serde_json::from_str::<Value>(raw) else {
            continue;
        };
        if pack["version"].as_str() == Some(duskweave::m1_https::VERSION_HTTPS_LOOP)
            && pack["references"].is_array()
        {
            return Ok(pack);
        }
    }
    Err("missing_or_malformed_https_context_pack")
}

pub(super) fn start_loopback_sdk_at(
    address: &str,
    dep_refresh: &'static str,
    d2_delay: Duration,
) -> Server {
    Server::start(address, move |inc| {
        let Ok(req) = serde_json::from_slice::<Value>(&inc.body) else {
            return Reply::Length(
                400,
                serde_json::to_vec(&json!({"error": "invalid_sdk_json"})).unwrap(),
            );
        };
        let pack = match extract_https_context_pack(&req) {
            Ok(p) => p,
            Err(code) => {
                return Reply::Length(400, serde_json::to_vec(&json!({"error": code})).unwrap());
            }
        };
        if !d2_delay.is_zero() {
            let refs = pack["references"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let is_d2 = refs.iter().any(|r| r["id"] == "endpoint_0")
                && !refs
                    .iter()
                    .any(|r| r["id"].as_str().is_some_and(|id| id.starts_with("https_")));
            if is_d2 {
                std::thread::sleep(d2_delay);
            }
        }
        let prop = lab_proposal_for_pack(&pack, dep_refresh);
        Reply::Length(200, serde_json::to_vec(&envelope(&prop)).unwrap())
    })
}

pub(super) fn start_loopback_sdk(dep_refresh: &'static str) -> Server {
    start_loopback_sdk_at("127.0.0.1:18083", dep_refresh, Duration::ZERO)
}

pub(super) fn start_loopback_sdk_with_delay(
    dep_refresh: &'static str,
    d2_delay: Duration,
) -> Server {
    start_loopback_sdk_at("127.0.0.1:18083", dep_refresh, d2_delay)
}

pub(super) fn build_native_run_bundle(
    eng: Uuid,
    cmp: Uuid,
    run_id: Uuid,
    manifest: &Value,
    now_s: i64,
    deadline_ms: i64,
) -> (Value, Value) {
    let qref = Uuid::parse_str(manifest["qualification_ref"].as_str().unwrap()).unwrap();
    let rel = manifest["release_sha256"].as_str().unwrap();
    let reg = lab_registration_json(eng, cmp, qref, rel, now_s);
    let req = lab_run_request_json(&reg, run_id, qref, rel, deadline_ms);
    (reg, req)
}

#[cfg(target_os = "linux")]
pub(super) mod linux_host {
    use super::*;
    use crate::lab_support::{validate_deployment_manifest, verify_responder_provenance_bytes};
    use crate::sdk_fixture::FixtureChild;
    use duskweave::m1_inference_profile::{HTTPS_LAB_IPV4, HTTPS_LAB_RESOLVER};
    use postgres::{Client, NoTls};
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Output, Stdio};
    use std::time::{Duration, Instant};

    pub(crate) fn cmd(binary: &str, args: &[&str]) -> Output {
        let ch = Command::new(binary)
            .args(args)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("host command");
        let mut fc = FixtureChild::new(ch);
        assert!(
            fc.wait_until(Instant::now() + Duration::from_secs(2))
                .is_some(),
            "host cmd timeout"
        );
        fc.output()
    }

    fn ip(args: &[&str]) -> Output {
        cmd("/usr/sbin/ip", args)
    }

    fn checked_ip(args: &[&str]) {
        let out = ip(args);
        assert!(
            out.status.success(),
            "ip failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn read_json<T: serde::de::DeserializeOwned>(p: impl AsRef<Path>) -> T {
        serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
    }

    pub(crate) struct NativeHost {
        pub admin: Client,
        pub manifest: Value,
        pub source_sha: String,
        pub environment: BTreeMap<String, String>,
        pub report: PathBuf,
        pub https_counters: PathBuf,
        build_root: PathBuf,
        ns: String,
        host_if: String,
        https_proc: Option<Child>,
        inputs: Vec<PathBuf>,
        ns_owned: bool,
        link_owned: bool,
        route_dns_owned: bool,
        route_https_owned: bool,
        fwd_owned: u8,
    }

    impl NativeHost {
        pub fn new() -> Self {
            assert_eq!(rustix::process::geteuid().as_raw(), 0, "root required");
            assert_eq!(
                std::fs::read_to_string(OWNER_MARKER).unwrap().trim(),
                "DW-IMPLEMENT-M1-LIVE-DNS-GENAI"
            );
            let cfg = PathBuf::from(std::env::var_os("DW_M1_LAB_CONFIG_ROOT").unwrap());
            let rpt = PathBuf::from(std::env::var_os("DW_M1_LAB_REPORT_ROOT").unwrap());
            let build_root = PathBuf::from(
                std::env::var_os("DW_M1_LAB_BUILD_ROOT")
                    .unwrap_or_else(|| DEFAULT_BUILD_ROOT.into()),
            );
            assert!(cfg.is_absolute() && rpt.is_absolute() && build_root.is_absolute());
            let env_map: BTreeMap<String, String> = read_json(cfg.join("environment.json"));
            let test_map: BTreeMap<String, String> = read_json(cfg.join("test-environment.json"));
            let manifest: Value = read_json(DEPLOYMENT_MANIFEST);
            validate_deployment_manifest(&manifest).unwrap();
            let rsp_bin = std::fs::read(build_root.join("go-release/httpslab-arm64")).unwrap();
            let rsp_prov =
                std::fs::read(build_root.join("go-release/httpslab-arm64.provenance.json"))
                    .unwrap();
            let prov = verify_responder_provenance_bytes(&rsp_prov, &rsp_bin).unwrap();
            let source_sha = prov["build_source_commit"].as_str().unwrap().to_owned();
            let mut admin =
                Client::connect(&test_map["DW_TEST_ADMIN_DATABASE_URL"], NoTls).unwrap();
            let id: Uuid = admin
                .query_one("SELECT gen_random_uuid()", &[])
                .unwrap()
                .get(0);
            let tag = &id.simple().to_string()[..10];
            let report = rpt.join(id.simple().to_string());
            std::fs::create_dir_all(&report).unwrap();
            for cidr in ["192.0.2.53/32", "198.51.100.10/32"] {
                assert!(
                    String::from_utf8_lossy(&ip(&["-4", "route", "show", cidr]).stdout)
                        .trim()
                        .is_empty(),
                    "route collision"
                );
            }
            Self {
                admin,
                manifest,
                source_sha,
                environment: env_map,
                https_counters: report.join("https-counters.json"),
                report,
                build_root,
                ns: format!("hl{tag}"),
                host_if: format!("hh{tag}"),
                https_proc: None,
                inputs: Vec::new(),
                ns_owned: false,
                link_owned: false,
                route_dns_owned: false,
                route_https_owned: false,
                fwd_owned: 0,
            }
        }

        pub fn start_topology_for(
            &mut self,
            scenario: &str,
            run_id: Uuid,
            dns_ttl: u32,
            dns_refresh_ttl: u32,
        ) {
            let inner = self.host_if.replacen("hh", "hn", 1);
            checked_ip(&["netns", "add", &self.ns]);
            self.ns_owned = true;
            checked_ip(&[
                "link",
                "add",
                &self.host_if,
                "type",
                "veth",
                "peer",
                "name",
                &inner,
            ]);
            self.link_owned = true;
            for args in [
                &["link", "set", inner.as_str(), "netns", &self.ns][..],
                &["addr", "add", "192.0.2.1/32", "dev", &self.host_if],
                &["link", "set", &self.host_if, "up"],
            ] {
                checked_ip(args);
            }
            for args in [
                &["addr", "add", "192.0.2.53/32", "dev", &inner][..],
                &["addr", "add", "198.51.100.10/32", "dev", &inner],
                &["link", "set", inner.as_str(), "up"],
                &["link", "set", "lo", "up"],
                &["route", "add", "192.0.2.1/32", "dev", inner.as_str()],
                &[
                    "route",
                    "add",
                    "10.203.0.0/24",
                    "via",
                    "192.0.2.1",
                    "dev",
                    inner.as_str(),
                    "onlink",
                ],
            ] {
                self.ns(args);
            }
            checked_ip(&["route", "add", "192.0.2.53/32", "dev", &self.host_if]);
            self.route_dns_owned = true;
            checked_ip(&["route", "add", "198.51.100.10/32", "dev", &self.host_if]);
            self.route_https_owned = true;
            for idx in 0..4u8 {
                assert!(self.iptables_rule("-I", idx));
                self.fwd_owned += 1;
            }
            let rsp = self.build_root.join("go-release/httpslab-arm64");
            self.https_proc = Some(
                Command::new("/usr/sbin/ip")
                    .args(["netns", "exec", &self.ns, rsp.to_str().unwrap()])
                    .args(["-listen", &format!("{HTTPS_LAB_IPV4}:443")])
                    .args(["-dns-listen", &format!("{HTTPS_LAB_RESOLVER}:53")])
                    .args([
                        "-dns-ttl",
                        &dns_ttl.to_string(),
                        "-dns-refresh-ttl",
                        &dns_refresh_ttl.to_string(),
                    ])
                    .args(["-scenario", scenario, "-run-id", &run_id.to_string()])
                    .args(["-counters", self.https_counters.to_str().unwrap()])
                    .env_clear()
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let dl = Instant::now() + Duration::from_secs(3);
            while !self.https_counters.exists() && Instant::now() < dl {
                let alive = self
                    .https_proc
                    .as_mut()
                    .unwrap()
                    .try_wait()
                    .unwrap()
                    .is_none();
                assert!(alive, "httpslab exited");
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(
                self.https_counters.exists(),
                "httpslab counter readiness timeout"
            );
        }

        fn ns(&self, args: &[&str]) {
            let mut full = vec!["netns", "exec", &self.ns, "/usr/sbin/ip"];
            full.extend_from_slice(args);
            checked_ip(&full);
        }

        fn iptables_rule(&self, op: &str, idx: u8) -> bool {
            let (proto, cidr, pflag, port, out) = match idx {
                0 => ("udp", "192.0.2.53/32", "--dport", "53", false),
                1 => ("udp", "192.0.2.53/32", "--sport", "53", true),
                2 => ("tcp", "198.51.100.10/32", "--dport", "443", false),
                _ => ("tcp", "198.51.100.10/32", "--sport", "443", true),
            };
            let (in_if, out_if, src, dst) = if out {
                (self.host_if.as_str(), "dvh+", cidr, "10.203.0.0/24")
            } else {
                ("dvh+", self.host_if.as_str(), "10.203.0.0/24", cidr)
            };
            let ch = Command::new("/usr/sbin/iptables")
                .args([op, "DOCKER-USER", "-i", in_if, "-o", out_if])
                .args(["-s", src, "-d", dst, "-p", proto, pflag, port])
                .args(["-m", "comment", "--comment", &self.ns, "-j", "ACCEPT"])
                .env_clear()
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("iptables");
            let mut fc = FixtureChild::new(ch);
            assert!(
                fc.wait_until(Instant::now() + Duration::from_secs(2))
                    .is_some(),
                "iptables timeout"
            );
            fc.output().status.success()
        }

        pub fn id(&mut self) -> Uuid {
            self.admin
                .query_one("SELECT gen_random_uuid()", &[])
                .unwrap()
                .get(0)
        }

        pub fn db_now_s(&mut self) -> i64 {
            self.admin
                .query_one(
                    "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
                    &[],
                )
                .unwrap()
                .get(0)
        }

        pub fn write_input(&mut self, value: &Value) -> PathBuf {
            use std::os::unix::fs::PermissionsExt;
            let p = Path::new("/run/duskweave/m1").join(format!("{}.input", self.id().simple()));
            std::fs::write(&p, serde_json::to_vec(value).unwrap()).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
            let uid = self.manifest["core_uid"].as_u64().unwrap() as u32;
            rustix::fs::chown(&p, Some(rustix::process::Uid::from_raw(uid)), None).unwrap();
            self.inputs.push(p.clone());
            p
        }

        pub fn cli(&self, args: &[&str]) -> Output {
            let mut ch = FixtureChild::new(
                Command::new("/usr/sbin/runuser")
                    .args(["-u", "duskweave-core", "--", CORE_BIN])
                    .args(args)
                    .env_clear()
                    .envs(&self.environment)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap(),
            );
            assert!(
                ch.wait_until(Instant::now() + Duration::from_secs(90))
                    .is_some()
            );
            ch.output()
        }

        pub fn receipt(out: &Output) -> Value {
            let txt = String::from_utf8(out.stdout.clone()).unwrap();
            serde_json::from_str(
                txt.lines()
                    .rev()
                    .find(|l| l.starts_with('{'))
                    .unwrap_or(&txt),
            )
            .unwrap()
        }

        pub fn dns_count(&self, k: &str) -> u64 {
            self.https_counters()[k].as_u64().unwrap_or(0)
        }

        pub fn https_counters(&self) -> Value {
            serde_json::from_slice(&std::fs::read(&self.https_counters).unwrap()).unwrap()
        }

        pub fn save_inspect_artifact(
            &self,
            label: &str,
            run_id: Uuid,
            inspect_stdout: &str,
            inspect_sanitized: &Value,
            recover_sanitized: &Value,
        ) {
            assert!(
                !inspect_stdout.contains("://")
                    && !inspect_stdout.contains("password=")
                    && !inspect_stdout.contains("DW_"),
                "unsafe inspect stdout"
            );
            let artifact = json!({
                "label": label,
                "source_sha": self.source_sha,
                "run_id": run_id,
                "counter_file": self.https_counters.display().to_string(),
                "counters": self.https_counters(),
                "inspect_stdout": inspect_stdout.trim(),
                "inspect_sanitized": inspect_sanitized,
                "recover_sanitized": recover_sanitized,
            });
            std::fs::write(
                self.report.join(format!("{label}-inspect-artifact.json")),
                serde_json::to_vec_pretty(&artifact).unwrap(),
            )
            .unwrap();
        }
    }

    impl Drop for NativeHost {
        fn drop(&mut self) {
            if let Some(mut p) = self.https_proc.take() {
                let _ = p.kill();
                let _ = p.wait();
            }
            let mut clean = (0..self.fwd_owned).all(|idx| self.iptables_rule("-D", idx));
            let ip_cleanups: [(bool, &[&str]); 4] = [
                (
                    self.route_https_owned,
                    &["route", "del", "198.51.100.10/32", "dev", &self.host_if],
                ),
                (
                    self.route_dns_owned,
                    &["route", "del", "192.0.2.53/32", "dev", &self.host_if],
                ),
                (self.link_owned, &["link", "del", &self.host_if]),
                (self.ns_owned, &["netns", "del", &self.ns]),
            ];
            for (owned, args) in ip_cleanups {
                if owned {
                    clean &= cmd("/usr/sbin/ip", args).status.success();
                }
            }
            for p in &self.inputs {
                let _ = std::fs::remove_file(p);
            }
            if !std::thread::panicking() {
                assert!(clean, "scoped LAB disposal incomplete");
            }
        }
    }
}
