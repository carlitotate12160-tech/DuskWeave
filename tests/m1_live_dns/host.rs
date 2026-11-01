//! Owned Oracle LAB fixture. Privileged setup is distinct from the installed core consumer.
#![cfg(target_os = "linux")]
use postgres::{Client, NoTls};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use uuid::Uuid;
const REPORT: &str =
    "/srv/duskweave/reports/live-dns-20261030/completion-20261031a/pr139-fix-20261101/restart-r2";
const CONFIG: &str = "/etc/duskweave/m1/lab-pr139-r2-20261101a";
pub const CORE: &str = "/usr/local/libexec/duskweave/duskweave";

pub fn command(binary: &str, args: &[&str]) -> std::process::Output {
    let child = Command::new(binary)
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("owned host command");
    let mut child = super::sdk_fixture::FixtureChild::new(child);
    assert!(
        child
            .wait_until(Instant::now() + Duration::from_secs(1))
            .is_some(),
        "bounded owned fixture command"
    );
    child.output()
}
fn checked(binary: &str, args: &[&str]) {
    assert!(
        command(binary, args).status.success(),
        "owned LAB setup/cleanup failed"
    );
}
pub fn root() {
    assert_eq!(
        rustix::process::geteuid().as_raw(),
        0,
        "explicit owned-host root qualification required"
    );
    assert_eq!(
        std::fs::read_to_string("/srv/duskweave/.live-dns-20261030-owner")
            .unwrap()
            .trim(),
        "DW-IMPLEMENT-M1-LIVE-DNS-GENAI"
    );
}
pub struct Host {
    pub admin: Client,
    pub manifest: Value,
    pub environment: BTreeMap<String, String>,
    pub report: PathBuf,
    pub counters: PathBuf,
    namespace: String,
    host: String,
    resolver: Option<std::process::Child>,
    input: Vec<PathBuf>,
    namespace_created: bool,
    link_created: bool,
    forward_owned: u8,
}
impl Host {
    pub fn new() -> Self {
        root();
        let environment: BTreeMap<String, String> =
            serde_json::from_slice(&std::fs::read(format!("{CONFIG}/environment.json")).unwrap())
                .unwrap();
        let test: BTreeMap<String, String> = serde_json::from_slice(
            &std::fs::read(format!("{CONFIG}/test-environment.json")).unwrap(),
        )
        .unwrap();
        let mut admin = Client::connect(&test["DW_TEST_ADMIN_DATABASE_URL"], NoTls)
            .expect("fresh owned database fixture");
        let id: Uuid = admin
            .query_one("SELECT gen_random_uuid()", &[])
            .unwrap()
            .get(0);
        let tag = &id.simple().to_string()[..12];
        let report = Path::new(REPORT).join(id.simple().to_string());
        std::fs::create_dir_all(&report).unwrap();
        let manifest =
            serde_json::from_slice(&std::fs::read("/etc/duskweave/m1/deployment.json").unwrap())
                .unwrap();
        let host = Self {
            admin,
            manifest,
            environment,
            counters: report.join("counters.json"),
            report,
            namespace: format!("dl{tag}"),
            host: format!("lh{tag}"),
            resolver: None,
            input: vec![],
            namespace_created: false,
            link_created: false,
            forward_owned: 0,
        };
        assert!(
            String::from_utf8(
                command("/usr/sbin/ip", &["-4", "route", "show", "192.0.2.53/32"]).stdout
            )
            .unwrap()
            .trim()
            .is_empty(),
            "LAB route collision"
        );
        host
    }
    pub fn id(&mut self) -> Uuid {
        self.admin
            .query_one("SELECT gen_random_uuid()", &[])
            .unwrap()
            .get(0)
    }
    pub fn write(&mut self, value: &Value) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = Path::new("/run/duskweave/m1").join(format!("{}.input", self.id().simple()));
        std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let uid = self.manifest["core_uid"].as_u64().unwrap() as u32;
        rustix::fs::chown(&path, Some(rustix::process::Uid::from_raw(uid)), None).unwrap();
        self.input.push(path.clone());
        path
    }
    pub fn spawn(&self, args: &[&str]) -> super::sdk_fixture::FixtureChild {
        let mut command = Command::new("/usr/sbin/runuser");
        command
            .args(["-u", "duskweave-core", "--", CORE])
            .args(args)
            .env_clear()
            .envs(&self.environment)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        super::sdk_fixture::FixtureChild::new(command.spawn().unwrap())
    }
    pub fn cli(&self, args: &[&str]) -> std::process::Output {
        let mut child = self.spawn(args);
        assert!(
            child
                .wait_until(Instant::now() + Duration::from_secs(65))
                .is_some(),
            "normal installed CLI deadline"
        );
        child.output()
    }
    pub fn receipt(output: &std::process::Output) -> Value {
        let text = String::from_utf8(output.stdout.clone()).unwrap();
        serde_json::from_str(
            text.lines()
                .rev()
                .find(|l| l.starts_with('{'))
                .unwrap_or_else(|| panic!("normal CLI receipt missing: {text}")),
        )
        .unwrap()
    }
    pub fn start_resolver(&mut self) {
        let inner = self.host.replacen("lh", "ln", 1);
        checked("/usr/sbin/ip", &["netns", "add", &self.namespace]);
        self.namespace_created = true;
        checked(
            "/usr/sbin/ip",
            &[
                "link", "add", &self.host, "type", "veth", "peer", "name", &inner,
            ],
        );
        self.link_created = true;
        checked(
            "/usr/sbin/ip",
            &["link", "set", &inner, "netns", &self.namespace],
        );
        checked(
            "/usr/sbin/ip",
            &["addr", "add", "192.0.2.1/32", "dev", &self.host],
        );
        self.ns(&["addr", "add", "192.0.2.53/32", "dev", &inner]);
        checked("/usr/sbin/ip", &["link", "set", &self.host, "up"]);
        self.ns(&["link", "set", &inner, "up"]);
        self.ns(&["link", "set", "lo", "up"]);
        checked(
            "/usr/sbin/ip",
            &["route", "add", "192.0.2.53/32", "dev", &self.host],
        );
        self.ns(&["route", "add", "192.0.2.1/32", "dev", &inner]);
        self.ns(&[
            "route",
            "add",
            "10.203.0.0/24",
            "via",
            "192.0.2.1",
            "dev",
            &inner,
            "onlink",
        ]);
        for direction in [false, true] {
            assert!(
                self.forward_rule("-I", direction),
                "scoped LAB forward rule"
            );
            self.forward_owned += 1;
        }
        let process = Command::new("/usr/sbin/ip")
            .args([
                "netns",
                "exec",
                &self.namespace,
                "/srv/duskweave-build/go-release/dns-lab",
                "--counter-file",
            ])
            .arg(&self.counters)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        self.resolver = Some(process);
        let end = Instant::now() + Duration::from_secs(2);
        while !self.counters.exists() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            self.count("ready"),
            1,
            "all owned resolver/trap listeners ready"
        );
        assert_eq!(self.count("dns_udp"), 0, "independent zero-send witness");
    }
    fn ns(&self, args: &[&str]) {
        let mut fixed = vec!["netns", "exec", self.namespace.as_str(), "/usr/sbin/ip"];
        fixed.extend_from_slice(args);
        checked("/usr/sbin/ip", &fixed);
    }
    fn forward_rule(&self, operation: &str, direction: bool) -> bool {
        let (input, output, source, destination, port) = if direction {
            (
                self.host.as_str(),
                "dvh+",
                "192.0.2.53/32",
                "10.203.0.0/24",
                "--sport",
            )
        } else {
            (
                "dvh+",
                self.host.as_str(),
                "10.203.0.0/24",
                "192.0.2.53/32",
                "--dport",
            )
        };
        command(
            "/usr/sbin/iptables",
            &[
                operation,
                "DOCKER-USER",
                "-i",
                input,
                "-o",
                output,
                "-s",
                source,
                "-d",
                destination,
                "-p",
                "udp",
                port,
                "53",
                "-m",
                "comment",
                "--comment",
                &self.namespace,
                "-j",
                "ACCEPT",
            ],
        )
        .status
        .success()
    }
    pub fn crash_owned_process(&self, run: Uuid, component: &str) {
        let core = phase_pid(run);
        let root = installed_launchers();
        let launcher = *root
            .iter()
            .find(|pid| {
                descends_from(**pid, core)
                    && parent_of(**pid).is_some_and(|parent| !root.contains(&parent))
            })
            .expect("the qualified launcher belongs to this acquisition child");
        let pid = match component {
            "core" => core,
            "launcher" => launcher,
            "watchdog" => *root
                .iter()
                .find(|pid| parent_of(**pid) == Some(launcher))
                .expect("independent owned watchdog"),
            _ => panic!("unsupported owned-process fault"),
        };
        let expected = if component == "core" {
            CORE
        } else {
            "/usr/local/libexec/duskweave/dw-boundary-launcher"
        };
        assert_eq!(
            std::fs::read_link(format!("/proc/{pid}/exe")).unwrap(),
            Path::new(expected)
        );
        checked("/bin/kill", &["-KILL", &pid.to_string()]);
    }
    pub fn workers_absent(&self, attempt: Uuid) -> bool {
        let output = command(
            "/usr/bin/docker",
            &[
                "ps",
                "-a",
                "--filter",
                &format!("label=org.duskweave.attempt={attempt}"),
                "--format",
                "{{.ID}}",
            ],
        );
        output.status.success() && output.stdout.iter().all(u8::is_ascii_whitespace)
    }
    pub fn probe_namespace(&self, attempt: Uuid, exporter: bool) -> Value {
        let name = format!(
            "dw{}{}",
            if exporter { "exp" } else { "cap" },
            attempt.simple()
        );
        let output = command(
            "/usr/bin/docker",
            &["inspect", "--format", "{{.State.Pid}}", &name],
        );
        assert!(
            output.status.success(),
            "exact owned worker must exist for the probe"
        );
        let pid = String::from_utf8(output.stdout).unwrap();
        assert!(pid.trim().parse::<u32>().unwrap() > 0);
        let output = command(
            "/usr/bin/nsenter",
            &[
                "-t",
                pid.trim(),
                "-n",
                "--",
                "/srv/duskweave-build/go-release/dns-lab",
                "--probe",
            ],
        );
        assert!(
            output.status.success(),
            "fixed probes require an isolated worker network namespace"
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
    pub fn private_bytes(&mut self, bytes: &[u8]) -> PathBuf {
        let path = self.write(&Value::Null);
        std::fs::write(&path, bytes).unwrap();
        path
    }
    pub fn count(&self, category: &str) -> u64 {
        let value: Value = serde_json::from_slice(&std::fs::read(&self.counters).unwrap()).unwrap();
        value[category].as_u64().unwrap_or(0)
    }
    pub fn live_binding(&self) -> Value {
        json!({"qualification_ref":self.manifest["qualification_ref"],
            "release_sha256":self.manifest["release_sha256"],"resolver_ipv4":"192.0.2.53"})
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        if let Some(mut process) = self.resolver.take() {
            let _ = process.kill();
            let end = Instant::now() + Duration::from_secs(1);
            while process.try_wait().ok().flatten().is_none() && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        let mut clean = true;
        for n in 0..self.forward_owned {
            clean &= self.forward_rule("-D", n == 1);
        }
        if self.link_created {
            clean &= command("/usr/sbin/ip", &["link", "del", &self.host])
                .status
                .success();
        }
        if self.namespace_created {
            clean &= command("/usr/sbin/ip", &["netns", "del", &self.namespace])
                .status
                .success();
        }
        if !std::thread::panicking() {
            assert!(clean, "owned LAB fixture disposal");
        }
        for path in &self.input {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn phase_pid(run: Uuid) -> u32 {
    let expected = run.to_string();
    for entry in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if std::fs::read_link(entry.path().join("exe")).ok().as_deref() != Some(Path::new(CORE)) {
            continue;
        }
        let Ok(args) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let args: Vec<_> = args.split(|b| *b == 0).collect();
        if args
            .windows(2)
            .any(|v| v == [b"--run".as_slice(), expected.as_bytes()])
            && args
                .windows(2)
                .any(|v| v == [b"--phase".as_slice(), b"acquisition".as_slice()])
        {
            return pid;
        }
    }
    panic!("owned acquisition process unavailable")
}
fn parent_of(pid: u32) -> Option<u32> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let mut rest = text.get(text.rfind(')')? + 1..)?.split_whitespace();
    if rest.next()? == "Z" {
        return None;
    }
    rest.next()?.parse().ok()
}
fn descends_from(mut pid: u32, parent: u32) -> bool {
    for _ in 0..16 {
        if pid == parent {
            return true;
        }
        let Some(next) = parent_of(pid) else {
            return false;
        };
        pid = next;
    }
    false
}
fn installed_launchers() -> Vec<u32> {
    std::fs::read_dir("/proc")
        .unwrap()
        .flatten()
        .filter_map(|entry| {
            if std::fs::read_link(entry.path().join("exe")).ok().as_deref()
                != Some(Path::new(
                    "/usr/local/libexec/duskweave/dw-boundary-launcher",
                ))
            {
                return None;
            }
            entry.file_name().to_string_lossy().parse().ok()
        })
        .collect()
}
