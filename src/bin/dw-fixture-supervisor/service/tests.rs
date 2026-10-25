#[cfg(test)]
mod scenarios {
    use super::super::*;

    fn inspect(ports: Value) -> Value {
        serde_json::json!([{
            "Config": { "Image": IMAGE },
            "State": { "Running": true },
            "NetworkSettings": { "Ports": ports },
        }])
    }

    #[test]
    fn qualified_loopback_binding_is_accepted() {
        let ports = json!({ "5432/tcp": [ { "HostIp": "127.0.0.1", "HostPort": "5432" } ] });
        assert!(evaluate_inspect(&inspect(ports)));
    }

    #[test]
    fn null_or_absent_port_metadata_is_refused_without_panic() {
        let variants = [
            Value::Null,
            json!({}),
            json!({ "5432/tcp": null }),
            json!({ "5432/tcp": [] }),
        ];
        for ports in variants {
            assert!(!evaluate_inspect(&inspect(ports.clone())), "{ports}");
        }
        assert!(!evaluate_inspect(&json!(null)));
        assert!(!evaluate_inspect(&json!([{}])));
    }

    #[test]
    fn foreign_binding_or_state_or_image_is_refused() {
        let remote = json!({ "5432/tcp": [ { "HostIp": "0.0.0.0", "HostPort": "5432" } ] });
        assert!(!evaluate_inspect(&inspect(remote)));
        let other_port = json!({ "5432/tcp": [ { "HostIp": "127.0.0.1", "HostPort": "5433" } ] });
        assert!(!evaluate_inspect(&inspect(other_port)));
        let mut stopped =
            inspect(json!({ "5432/tcp": [ { "HostIp": "127.0.0.1", "HostPort": "5432" } ] }));
        stopped[0]["State"]["Running"] = json!(false);
        assert!(!evaluate_inspect(&stopped));
        let mut wrong_image =
            inspect(json!({ "5432/tcp": [ { "HostIp": "127.0.0.1", "HostPort": "5432" } ] }));
        wrong_image[0]["Config"]["Image"] = json!("postgres:latest");
        assert!(!evaluate_inspect(&wrong_image));
    }

    #[test]
    fn version_expectations_are_closed() {
        let options = |prefix: Option<&str>, exact: Option<&str>| ServiceOptions {
            container: "dwpg-r3".into(),
            ci: false,
            version_prefix: prefix.map(str::to_string),
            version_exact: exact.map(str::to_string),
        };
        assert!(version_matches("17.11", &options(Some("17."), None)));
        assert!(!version_matches("16.2", &options(Some("17."), None)));
        assert!(version_matches("17.11", &options(None, Some("17.11"))));
        assert!(!version_matches(
            "17.11.2 compiled",
            &options(None, Some("17.11"))
        ));
        assert!(!version_matches("", &options(Some("17."), None)));
    }

    #[test]
    fn only_owned_containers_are_allowed() {
        assert!(container_allowed("dwpg-r3", false));
        assert!(container_allowed("dwpg", false));
        assert!(container_allowed("dwpg", true));
        assert!(!container_allowed("dwpg-r3", true));
        assert!(!container_allowed("other", false));
    }

    #[test]
    fn flag_parsing_is_closed() {
        let arguments = ["--container", "dwpg-r3", "--version-prefix", "17."].map(String::from);
        let options = parse_options(&arguments).expect("options");
        assert_eq!(options.container, "dwpg-r3");
        assert!(!options.ci);
        let ci = ["--container", "dwpg", "--ci", "--version-exact", "17.11"].map(String::from);
        assert!(parse_options(&ci).is_ok());
        assert!(parse_options(&["--unknown".to_string()]).is_err());
        assert!(parse_options(&["--container".to_string()]).is_err());
    }

    fn qualified_inspect(_: &str) -> Option<Vec<u8>> {
        let ports = json!({ "5432/tcp": [ { "HostIp": "127.0.0.1", "HostPort": "5432" } ] });
        Some(
            json!([{ "Config": { "Image": IMAGE }, "State": { "Running": true },
                     "NetworkSettings": { "Ports": ports } }])
            .to_string()
            .into_bytes(),
        )
    }

    fn fixed_version(_: &str) -> Option<String> {
        Some("17.11 (Debian 17.11-1.pgdg13+2)".into())
    }

    fn absent_probe(_: &str) -> Option<Vec<u8>> {
        None
    }

    fn absent_version(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn qualified_service_reports_only_safe_metadata() {
        let arguments = ["--container", "dwpg-r3", "--version-exact", "17.11"].map(String::from);
        let value = command_with(&arguments, qualified_inspect, fixed_version).expect("qualified");
        assert_eq!(value["ok"], true);
        assert_eq!(value["container"], "dwpg-r3");
        assert_eq!(value["server_version"], "17.11 (Debian 17.11-1.pgdg13+2)");
        let prefix = ["--container", "dwpg-r3", "--version-prefix", "17."].map(String::from);
        assert!(command_with(&prefix, qualified_inspect, fixed_version).is_ok());
    }

    #[test]
    fn version_mismatch_or_unreachable_docker_is_refused() {
        let mismatch = ["--container", "dwpg-r3", "--version-exact", "18.0"].map(String::from);
        assert!(command_with(&mismatch, qualified_inspect, fixed_version).is_err());
        let unreachable = ["--container", "dwpg-r3", "--version-prefix", "17."].map(String::from);
        assert!(command_with(&unreachable, absent_probe, fixed_version).is_err());
        let no_version = ["--container", "dwpg-r3", "--version-prefix", "17."].map(String::from);
        assert!(command_with(&no_version, qualified_inspect, absent_version).is_err());
    }

    #[test]
    fn service_capture_settles_pipe_holding_descendant() {
        let root = std::env::temp_dir().join(format!("dw-probe-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("root");
        let python = std::env::var("DW_TEST_PYTHON")
            .unwrap_or_else(|_| String::from(if cfg!(windows) { "python" } else { "python3" }));
        let script = format!(
            "import subprocess, sys; from pathlib import Path; \
             p = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(6)']); \
             Path({root:?}).joinpath('pid').write_text(str(p.pid)); print('probe-result')"
        );
        let started = std::time::Instant::now();
        let output = bounded_output(&[&python, "-c", &script]);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(4),
            "service capture exceeded settlement bound: {:?}",
            started.elapsed()
        );
        assert_eq!(
            output,
            Some(b"probe-result\r\n".to_vec())
                .filter(|_| cfg!(windows))
                .or_else(|| Some(b"probe-result\n".to_vec()))
        );
        let pid = std::fs::read_to_string(root.join("pid")).expect("pid");
        let alive = if cfg!(windows) {
            let output = Command::new("tasklist")
                .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
                .output()
                .expect("independent observation");
            String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
        } else {
            std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .is_ok_and(|stat| !stat.contains(") Z "))
        };
        assert!(!alive, "owned descendant survived");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn service_capture_refuses_overflow_and_nonzero_probe() {
        let python = std::env::var("DW_TEST_PYTHON")
            .unwrap_or_else(|_| String::from(if cfg!(windows) { "python" } else { "python3" }));
        assert!(bounded_output(&[&python, "-c", "raise SystemExit(7)"]).is_none());
        assert!(bounded_output(&[&python, "-c", "print('x' * (17 * 1024 * 1024))"]).is_none());
    }
}
