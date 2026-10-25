//! Staged predecessor fixture custody and immutable upgrade assertions.
use crate::native;
use serde_json::Value;
use std::time::{Duration, Instant};

pub fn assert_predecessor_catalog() {
    let row = native::admin()
        .query_one(
            "SELECT to_regclass('execution.inference_decision_windows') IS NULL,
         to_regprocedure('execution.successor_configuration(jsonb)') IS NULL
         AND to_regprocedure('execution.assert_successor_pack(jsonb,jsonb,jsonb)') IS NULL,
         to_regclass('execution.run_phase_diagnostics') IS NOT NULL,
         pg_get_constraintdef(oid) FROM pg_constraint
         WHERE conrelid='trajectory.reasoning_history'::regclass
         AND conname='reasoning_history_version_check'",
            &[],
        )
        .unwrap();
    assert!(
        row.get::<_, bool>(0),
        "T8 must start before 0021: successor table already exists"
    );
    assert!(
        row.get::<_, bool>(1),
        "T8 successor function already exists"
    );
    assert!(
        row.get::<_, bool>(2),
        "T8 requires the 0020 diagnostic catalog"
    );
    let constraint: String = row.get(3);
    assert!(
        constraint.contains("version") && constraint.contains("1") && !constraint.contains("2"),
        "T8 requires the predecessor history version constraint"
    );
}

const UPGRADE_TEST: &str =
    "t8_populated_legacy_owner_records_survive_forward_migration_and_repeatable_refresh";
const UPGRADE_MARKER: &str = "DW_TEST_LIFECYCLE_UPGRADE_WORKER";

/// Reenter only this test in a fresh staged fixture using the already-built executable.
pub fn upgrade_worker() -> bool {
    if let Some(marker) = std::env::var_os(UPGRADE_MARKER) {
        assert_eq!(marker, "0020-worker-v1", "invalid private T8 child marker");
        return true;
    }
    let unique = format!(
        "{:x}_{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let database = format!("dw_native_lifecycle_upgrade_{unique}");
    let report = std::env::temp_dir().join(format!("dw-t8-upgrade-{unique}"));
    let python = std::env::var_os("DW_TEST_PYTHON").unwrap_or_else(|| {
        if cfg!(windows) {
            "python".into()
        } else {
            "python3".into()
        }
    });
    let mut command = std::process::Command::new(python);
    command
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env(UPGRADE_MARKER, "0020-worker-v1")
        .env_remove("DEEPSEEK_API_KEY")
        .arg("tests/support/m1_native_bootstrap.py")
        .args([
            "--container",
            if cfg!(windows) { "dwpg-r3" } else { "dwpg" },
        ]);
    if !cfg!(windows) {
        command.arg("--ci");
    }
    command
        .args([
            "--database",
            "fixture",
            "--database-name",
            &database,
            "--schema-revision",
            "0020",
            "--report",
        ])
        .arg(&report)
        .arg("--")
        .arg(std::env::current_exe().unwrap())
        .args([UPGRADE_TEST, "--exact", "--test-threads=1", "--nocapture"]);
    let output = bounded_upgrade_child(command);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "T8 staged fixture failed; report {}: {stdout}\n{stderr}",
        report.display()
    );
    assert!(
        stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored;")
            && stdout.contains("t8_upgrade_worker_complete"),
        "T8 worker must actually execute: {stdout}"
    );
    println!(
        "T8 staged upgrade passed; retained report {}",
        report.display()
    );
    false
}

fn bounded_upgrade_child(mut command: std::process::Command) -> std::process::Output {
    use std::io::Read;
    use std::process::Stdio;
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let deadline = Instant::now() + Duration::from_secs(240);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            // Custodian cleanup after failure is never product termination evidence.
            #[cfg(windows)]
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &child.id().to_string(), "/T", "/F"])
                .output();
            #[cfg(unix)]
            let _ = std::process::Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .output();
            let _ = child.kill();
            let _ = child.wait();
            panic!("T8 staged fixture custodian timeout; qualification failed");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    std::process::Output {
        status,
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
    }
}

/// Read actual immutable owner rows through fresh restricted connections.
pub fn legacy_snapshot() -> Value {
    let owners = [
        (
            "DW_INFERENCE_BROKER_DATABASE_URL",
            &[
                "execution.autonomous_runs",
                "execution.inference_calls",
                "execution.native_input_bindings",
                "execution.inference_reservations",
                "execution.inference_closures",
                "execution.fixture_dns_attempts",
                "execution.fixture_dns_results",
            ][..],
        ),
        (
            "DW_COGNITION_DATABASE_URL",
            &[
                "cognition.reasoning_decisions",
                "cognition.reasoning_outbox",
            ][..],
        ),
        (
            "DW_TRAJECTORY_DATABASE_URL",
            &["trajectory.reasoning_history"][..],
        ),
    ];
    let mut snapshot = serde_json::Map::new();
    for (owner, tables) in owners {
        let mut connection = native::client(owner);
        for table in tables {
            let rows: Value = connection.query_one(&format!(
                "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text),'[]') FROM {table} t"
            ), &[]).unwrap().get(0);
            snapshot.insert((*table).into(), rows);
        }
    }
    Value::Object(snapshot)
}
