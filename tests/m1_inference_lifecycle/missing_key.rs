//! Fresh key-free legacy profile admission and recovery.
use super::*;

pub(super) fn t5_deepseek_cli_without_a_key_keeps_its_profile_and_never_falls_back_or_replays() {
    let _guard = native::db();
    run_support::refresh();
    for (i, profile) in [
        "deepseek-responses-reasoning-v1",
        "deepseek-responses-low-v1",
    ]
    .into_iter()
    .enumerate()
    {
        let q = lifecycle::registered(0xe610 + i as u128, profile);
        let path = run_support::path(&q);
        let control = run_support::control_root();

        let diagnostic_file = std::fs::File::create(control.join("stderr.log")).unwrap();
        let launched = std::time::Instant::now();
        let child = run_support::command()
            .env("DW_M1_RUN_CONTROL_ROOT", &control)
            .args([
                "m1-run",
                "--input",
                path.to_str().unwrap(),
                "--timing-profile",
                "reasoning-bounded-v1",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::from(diagnostic_file))
            .spawn()
            .unwrap();
        let mut parent = run_support::OwnedRun::new(child, run_support::id(&q));
        parent.own_file(&path);
        parent.own_control_root(&control);
        // Keep the original stdout pipe capture while retaining scoped custody.
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut parent.stdout.take().unwrap(), &mut bytes).unwrap();
        let status = parent.wait().unwrap();
        assert!(!status.success(), "{profile}");
        let text = String::from_utf8_lossy(&bytes);
        let receipt: Value = text
            .lines()
            .find(|line| line.starts_with('{'))
            .map(|line| serde_json::from_str(line).unwrap())
            .unwrap_or_else(|| panic!("missing run receipt: {text}"));
        let diagnostic = &receipt["diagnostics"][0];
        let cause = match diagnostic["cause"].as_str() {
            Some("readiness_deadline") => "readiness_deadline",
            Some("child_error") => "child_error",
            Some("child_exit") => "child_exit",
            _ => "unavailable",
        };
        eprintln!(
            "safe_missing_key case={i} test_command_elapsed_ms={} ready={:?} cause={cause} decisions={:?} effects={:?} expected_disposition={}",
            launched.elapsed().as_millis(),
            diagnostic["readiness_observed"].as_bool(),
            receipt["decision_count"].as_u64(),
            receipt["effect_count"].as_u64(),
            receipt["recorded_decisions"][0]["disposition"] == "live_key_missing"
        );
        assert_eq!(receipt["inference_profile"], profile);
        assert_eq!(receipt["inference"], json!([]));
        assert_eq!(receipt["effect_count"], 0);
        assert_eq!(
            receipt["recorded_decisions"][0]["disposition"],
            "live_key_missing"
        );
        assert_eq!(
            receipt["recorded_decisions"][0]["current_use"],
            "not_admitted"
        );
        for _ in 0..2 {
            let (ok, after) =
                run_support::cli("m1-run-recover", "--run", q["run_id"].as_str().unwrap());
            assert!(ok, "{after}");
            assert_eq!(after["inference"], json!([]));
            assert_eq!(after["effect_count"], 0);
        }
    }
}
