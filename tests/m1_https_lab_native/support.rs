#![allow(dead_code)]
//! Shared config, responder provenance, scoped netns topology, and SQL/CLI helpers for native HTTPS LAB.

use crate::lab_support::{lab_proposal_for_pack, lab_registration_json, lab_run_request_json};
use crate::sdk_fixture::{Reply, Server, envelope};
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) const CORE_BIN: &str = "/usr/local/libexec/duskweave/duskweave";
pub(super) const DEFAULT_BUILD_ROOT: &str = "/srv/duskweave-build/https-lab-r1";
pub(super) const OWNER_MARKER: &str = "/etc/duskweave/m1/.live-dns-owner";
pub(super) const DEPLOYMENT_MANIFEST: &str = "/etc/duskweave/m1/deployment.json";

pub(super) fn start_loopback_sdk(dep_refresh: &'static str) -> Server {
    Server::lab(move |inc| {
        let req: Value = serde_json::from_slice(&inc.body).expect("SDK request JSON");
        let text = req["input"][0]["content"][0]["text"]
            .as_str()
            .unwrap_or("{}");
        let pack: Value = serde_json::from_str(text).unwrap_or_else(|_| json!({"references": []}));
        let prop = lab_proposal_for_pack(&pack, dep_refresh);
        Reply::Length(200, serde_json::to_vec(&envelope(&prop)).unwrap())
    })
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
    use duskweave::m1_inference_profile::HTTPS_LAB_IPV4;
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

    fn checked(binary: &str, args: &[&str]) {
        let out = cmd(binary, args);
        assert!(
            out.status.success(),
            "cmd {binary} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    pub(crate) struct NativeHost {
        pub admin: Client,
        pub manifest: Value,
        pub environment: BTreeMap<String, String>,
        pub report: PathBuf,
        pub dns_counters: PathBuf,
        pub https_counters: PathBuf,
        build_root: PathBuf,
        ns: String,
        host_if: String,
        dns_proc: Option<Child>,
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
            let env_map: BTreeMap<String, String> =
                serde_json::from_slice(&std::fs::read(cfg.join("environment.json")).unwrap())
                    .unwrap();
            let test_map: BTreeMap<String, String> =
                serde_json::from_slice(&std::fs::read(cfg.join("test-environment.json")).unwrap())
                    .unwrap();
            let manifest: Value =
                serde_json::from_slice(&std::fs::read(DEPLOYMENT_MANIFEST).unwrap()).unwrap();
            validate_deployment_manifest(&manifest).unwrap();
            let rsp_bin = std::fs::read(build_root.join("go-release/httpslab-arm64")).unwrap();
            let rsp_prov =
                std::fs::read(build_root.join("go-release/httpslab-arm64.provenance.json"))
                    .unwrap();
            verify_responder_provenance_bytes(&rsp_prov, &rsp_bin).unwrap();
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
                let r = cmd("/usr/sbin/ip", &["-4", "route", "show", cidr]);
                assert!(
                    String::from_utf8_lossy(&r.stdout).trim().is_empty(),
                    "route collision"
                );
            }
            Self {
                admin,
                manifest,
                environment: env_map,
                dns_counters: report.join("dns-counters.json"),
                https_counters: report.join("https-counters.json"),
                report,
                build_root,
                ns: format!("hl{tag}"),
                host_if: format!("hh{tag}"),
                dns_proc: None,
                https_proc: None,
                inputs: Vec::new(),
                ns_owned: false,
                link_owned: false,
                route_dns_owned: false,
                route_https_owned: false,
                fwd_owned: 0,
            }
        }

        pub fn start_topology(&mut self, scenario: &str) {
            let inner = self.host_if.replacen("hh", "hn", 1);
            checked("/usr/sbin/ip", &["netns", "add", &self.ns]);
            self.ns_owned = true;
            checked(
                "/usr/sbin/ip",
                &[
                    "link",
                    "add",
                    &self.host_if,
                    "type",
                    "veth",
                    "peer",
                    "name",
                    &inner,
                ],
            );
            self.link_owned = true;
            checked("/usr/sbin/ip", &["link", "set", &inner, "netns", &self.ns]);
            checked(
                "/usr/sbin/ip",
                &["addr", "add", "192.0.2.1/32", "dev", &self.host_if],
            );
            self.ns(&["addr", "add", "192.0.2.53/32", "dev", &inner]);
            self.ns(&["addr", "add", "198.51.100.10/32", "dev", &inner]);
            checked("/usr/sbin/ip", &["link", "set", &self.host_if, "up"]);
            for args in [
                &["link", "set", inner.as_str(), "up"][..],
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
            checked(
                "/usr/sbin/ip",
                &["route", "add", "192.0.2.53/32", "dev", &self.host_if],
            );
            self.route_dns_owned = true;
            checked(
                "/usr/sbin/ip",
                &["route", "add", "198.51.100.10/32", "dev", &self.host_if],
            );
            self.route_https_owned = true;
            for idx in 0..4u8 {
                assert!(self.iptables_rule("-I", idx));
                self.fwd_owned += 1;
            }
            let dns_bin = std::env::var("DW_M1_LAB_DNS_BIN")
                .unwrap_or_else(|_| "/srv/duskweave-build/go-release/dns-lab".into());
            self.dns_proc = Some(
                Command::new("/usr/sbin/ip")
                    .args(["netns", "exec", &self.ns, &dns_bin, "--counter-file"])
                    .arg(&self.dns_counters)
                    .env_clear()
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let rsp_bin = self.build_root.join("go-release/httpslab-arm64");
            self.https_proc = Some(
                Command::new("/usr/sbin/ip")
                    .args([
                        "netns",
                        "exec",
                        &self.ns,
                        rsp_bin.to_str().unwrap(),
                        "-listen",
                        &format!("{HTTPS_LAB_IPV4}:443"),
                        "-scenario",
                        scenario,
                        "-counters",
                        self.https_counters.to_str().unwrap(),
                    ])
                    .env_clear()
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let dl = Instant::now() + Duration::from_secs(3);
            while !(self.dns_counters.exists() && self.https_counters.exists())
                && Instant::now() < dl
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(self.dns_counters.exists() && self.https_counters.exists());
        }

        fn ns(&self, args: &[&str]) {
            let mut full = vec!["netns", "exec", &self.ns, "/usr/sbin/ip"];
            full.extend_from_slice(args);
            checked("/usr/sbin/ip", &full);
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
            cmd(
                "/usr/sbin/iptables",
                &[
                    op,
                    "DOCKER-USER",
                    "-i",
                    in_if,
                    "-o",
                    out_if,
                    "-s",
                    src,
                    "-d",
                    dst,
                    "-p",
                    proto,
                    pflag,
                    port,
                    "-m",
                    "comment",
                    "--comment",
                    &self.ns,
                    "-j",
                    "ACCEPT",
                ],
            )
            .status
            .success()
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
            let v: Value =
                serde_json::from_slice(&std::fs::read(&self.dns_counters).unwrap()).unwrap();
            v[k].as_u64().unwrap_or(0)
        }

        pub fn https_counters(&self) -> Value {
            serde_json::from_slice(&std::fs::read(&self.https_counters).unwrap()).unwrap()
        }
    }

    impl Drop for NativeHost {
        fn drop(&mut self) {
            for proc in [self.https_proc.take(), self.dns_proc.take()]
                .into_iter()
                .flatten()
            {
                let mut p = proc;
                let _ = p.kill();
                let _ = p.wait();
            }
            let mut clean = true;
            for idx in 0..self.fwd_owned {
                clean &= self.iptables_rule("-D", idx);
            }
            if self.route_https_owned {
                clean &= cmd(
                    "/usr/sbin/ip",
                    &["route", "del", "198.51.100.10/32", "dev", &self.host_if],
                )
                .status
                .success();
            }
            if self.route_dns_owned {
                clean &= cmd(
                    "/usr/sbin/ip",
                    &["route", "del", "192.0.2.53/32", "dev", &self.host_if],
                )
                .status
                .success();
            }
            if self.link_owned {
                clean &= cmd("/usr/sbin/ip", &["link", "del", &self.host_if])
                    .status
                    .success();
            }
            if self.ns_owned {
                clean &= cmd("/usr/sbin/ip", &["netns", "del", &self.ns])
                    .status
                    .success();
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
