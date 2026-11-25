#![allow(dead_code)]
//! Shared config, responder provenance, private control-root isolation, and SQL/CLI helpers for native HTTPS LAB.

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
    use super::super::topology::resolve_core_gid_from_passwd;
    use super::super::topology_linux::LabTopology;
    use super::*;
    use crate::lab_support::{validate_deployment_manifest, verify_responder_provenance_bytes};
    use crate::sdk_fixture::FixtureChild;
    use postgres::{Client, NoTls};
    use rustix::fs::{Gid, Uid, chown};
    use std::collections::BTreeMap;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output, Stdio};
    use std::sync::{Mutex, MutexGuard};
    use std::time::{Duration, Instant};

    static HOST_SERIAL: Mutex<()> = Mutex::new(());

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
        core_uid: u32,
        core_gid: u32,
        topology: LabTopology,
        inputs: Vec<PathBuf>,
        _serial: MutexGuard<'static, ()>,
    }

    impl NativeHost {
        pub fn new() -> Self {
            let serial = HOST_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
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
            let mut env_map: BTreeMap<String, String> = read_json(cfg.join("environment.json"));
            let test_map: BTreeMap<String, String> = read_json(cfg.join("test-environment.json"));
            let manifest: Value = read_json(DEPLOYMENT_MANIFEST);
            validate_deployment_manifest(&manifest).unwrap();
            let rsp_bin = std::fs::read(build_root.join("go-release/httpslab-arm64")).unwrap();
            let rsp_prov =
                std::fs::read(build_root.join("go-release/httpslab-arm64.provenance.json"))
                    .unwrap();
            let prov = verify_responder_provenance_bytes(&rsp_prov, &rsp_bin).unwrap();
            let source_sha = prov["build_source_commit"].as_str().unwrap().to_owned();
            let core_uid = manifest["core_uid"].as_u64().unwrap() as u32;
            let passwd = std::fs::read_to_string("/etc/passwd").unwrap();
            let core_gid = resolve_core_gid_from_passwd(&passwd, core_uid).unwrap();
            let mut admin =
                Client::connect(&test_map["DW_TEST_ADMIN_DATABASE_URL"], NoTls).unwrap();
            let id: Uuid = admin
                .query_one("SELECT gen_random_uuid()", &[])
                .unwrap()
                .get(0);
            let tag = id.simple().to_string();
            let report = rpt.join(&tag);
            std::fs::create_dir_all(&report).unwrap();
            let topology = LabTopology::new(&tag[..12], core_uid, core_gid).unwrap();
            // Scope core control root to this NativeHost's child environment only.
            env_map.insert(
                "DW_M1_RUN_CONTROL_ROOT".into(),
                topology.dirs.core_control_dir.to_str().unwrap().to_owned(),
            );
            Self {
                admin,
                manifest,
                source_sha,
                environment: env_map,
                https_counters: report.join("https-counters.json"),
                report,
                build_root,
                core_uid,
                core_gid,
                topology,
                inputs: Vec::new(),
                _serial: serial,
            }
        }

        pub fn start_topology_for(
            &mut self,
            scenario: &str,
            run_id: Uuid,
            dns_ttl: u32,
            dns_refresh_ttl: u32,
        ) {
            let rsp = self.build_root.join("go-release/httpslab-arm64");
            self.topology
                .start_for(&rsp, scenario, run_id, dns_ttl, dns_refresh_ttl)
                .unwrap();
            let _ = self.https_counters();
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
            let input_id = self.id();
            let p = self
                .topology
                .dirs
                .core_input_dir
                .join(format!("{}.input", input_id.simple()));
            std::fs::write(&p, serde_json::to_vec(value).unwrap()).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
            chown(
                &p,
                Some(Uid::from_raw(self.core_uid)),
                Some(Gid::from_raw(self.core_gid)),
            )
            .unwrap();
            self.inputs.push(p.clone());
            p
        }

        pub fn prepare_run_input(
            &mut self,
            run_id: Uuid,
            mutate: impl FnOnce(&mut Value),
        ) -> PathBuf {
            let (e, c, now_s) = (self.id(), self.id(), self.db_now_s());
            let (reg, mut req) = build_native_run_bundle(
                e,
                c,
                run_id,
                &self.manifest,
                now_s,
                now_s * 1000 + 400_000,
            );
            mutate(&mut req);
            let op = self.id();
            let reg_file = self.write_input(&reg);
            let reg_out = self.cli(&[
                "register",
                "--operation",
                &op.to_string(),
                "--input",
                reg_file.to_str().unwrap(),
            ]);
            assert!(reg_out.status.success(), "register failed for {run_id}");
            self.write_input(&req)
        }

        pub fn spawn_cli(&self, args: &[&str]) -> FixtureChild {
            FixtureChild::new(
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
            )
        }

        pub fn cli(&self, args: &[&str]) -> Output {
            let mut ch = self.spawn_cli(args);
            assert!(
                ch.wait_until(Instant::now() + Duration::from_secs(90))
                    .is_some(),
                "cli command timed out: {:?}",
                args
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
            let bytes = std::fs::read(&self.topology.dirs.counter_path).unwrap();
            let val: Value = serde_json::from_slice(&bytes).unwrap();
            let _ = std::fs::write(&self.https_counters, &bytes);
            val
        }

        pub fn inject_control_root_loss(&mut self, suffix: &str) -> Result<PathBuf, String> {
            self.topology.dirs.inject_control_root_loss(suffix)
        }

        pub fn sql_snapshot(&mut self, run_id: Uuid) -> Result<Value, String> {
            self.admin
                .query_one(
                    "SELECT jsonb_build_object(
                        'control', (SELECT row_to_json(c) FROM execution.autonomous_control c WHERE c.run_id = $1),
                        'steps', coalesce((SELECT jsonb_agg(row_to_json(s) ORDER BY s.step_index)
                                           FROM execution.https_loop_steps s WHERE s.run_id = $1), '[]'::jsonb),
                        'sessions', coalesce((SELECT jsonb_agg(jsonb_build_object(
                                                'attempt', les.attempt, 'step_index', les.step_index,
                                                'attempt_kind', les.attempt_kind, 'generation', les.generation,
                                                'control_phase', lec.phase, 'granted_phase', lec.granted_phase,
                                                'possibly_started', lec.possibly_started,
                                                'disposal_confirmed', lec.disposal_confirmed
                                              ) ORDER BY les.generation)
                                              FROM execution.live_effect_sessions les
                                              JOIN execution.live_effect_control lec USING (attempt)
                                              WHERE les.run_id = $1), '[]'::jsonb),
                        'events', coalesce((SELECT jsonb_agg(jsonb_build_object(
                                                'attempt', lee.attempt, 'generation', lee.generation,
                                                'phase', lee.phase, 'occurred_at_ms', lee.occurred_at_ms
                                              ) ORDER BY lee.generation, lee.occurred_at_ms, lee.phase)
                                            FROM execution.live_effect_events lee WHERE lee.run_id = $1), '[]'::jsonb)
                     )::json",
                    &[&run_id],
                )
                .map(|r| r.get(0))
                .map_err(|e| e.to_string())
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
            self.save_structured_artifact(label, &artifact);
        }

        pub fn save_structured_artifact(&self, label: &str, artifact: &Value) {
            let encoded = serde_json::to_vec_pretty(artifact).unwrap();
            let text = std::str::from_utf8(&encoded).unwrap();
            assert!(
                !text.contains("://") && !text.contains("password=") && !text.contains("DW_"),
                "unsafe artifact payload"
            );
            std::fs::write(
                self.report.join(format!("{label}-inspect-artifact.json")),
                encoded,
            )
            .unwrap();
        }

        pub fn dispose(mut self) -> Result<(), Vec<String>> {
            for p in self.inputs.drain(..) {
                let _ = std::fs::remove_file(p);
            }
            self.topology.dispose()
        }
    }

    impl Drop for NativeHost {
        fn drop(&mut self) {
            for p in self.inputs.drain(..) {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}
