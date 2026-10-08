//! Narrow fixtures for the wait-bound integration suites: env-local CLI
//! command construction, supervised bounded execution, test-owned lock and
//! trigger faults, and a correctly escaped loopback keyword DSN for the
//! relay cases. Included as a module by the real test crates; not a
//! standalone target. Admin connections are fixture setup only; runtime
//! assertions use the restricted login. Fixture names carry the `dwwait_`
//! prefix and Drop removes only the fixture's own objects or lock.
#![allow(dead_code)]

use postgres::Client;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Measured CI scheduling margin above the fixed 5-second startup bound.
pub const STARTUP_ENVELOPE: Duration = Duration::from_secs(8);
/// Measured CI scheduling margin above the fixed 30-second invocation bound.
pub const COMMAND_ENVELOPE: Duration = Duration::from_secs(35);
/// Distinguishes the active watchdog from the 5-second startup refusal.
pub const COMMAND_FLOOR: Duration = Duration::from_secs(20);

/// A CLI process invocation bound to one supplied DSN through env-local
/// configuration; DW_DATABASE_URL_FILE is cleared so the source stays
/// unambiguous on both platforms.
pub fn cli(args: &[String], dsn: &str) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_duskweave"));
    cmd.env_clear();
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", profile);
    }
    cmd.env("DW_DATABASE_CONFIG_MODE", "env-local");
    cmd.env("DW_DATABASE_URL", dsn);
    cmd.args(args);
    cmd
}

/// Spawn with captured stdio so a test can inspect durable state while the
/// invocation is still alive.
pub fn spawn_piped(cmd: &mut Command) -> (Child, Instant) {
    let started = Instant::now();
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    (child, started)
}

/// Supervised child run: the bound only fails and cleans up a broken test;
/// the asserted status is always the CLI's own.
pub fn run_bounded(cmd: &mut Command, bound: Duration) -> (Output, Duration) {
    let started = Instant::now();
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let (output, _) = wait_bounded(child, bound, started);
    (output, started.elapsed())
}

/// Wait for a spawned child under a supervision bound; a bound breach kills
/// the broken child and fails the test rather than claiming a CLI timeout.
pub fn wait_bounded(child: Child, bound: Duration, started: Instant) -> (Output, Instant) {
    let mut child = child;
    loop {
        if child.try_wait().unwrap().is_some() {
            return (child.wait_with_output().unwrap(), started);
        }
        if started.elapsed() >= bound {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("CLI exceeded the {bound:?} supervision bound");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The watchdog's own stop signature: exit 124, no fabricated receipt and no
/// diagnostics through the locked stdio pair.
pub fn expect_watchdog_stop(
    output: &Output,
    elapsed: Duration,
    floor: Duration,
    envelope: Duration,
) {
    assert_eq!(
        output.status.code(),
        Some(124),
        "watchdog exit status, not a supervisor kill or business status"
    );
    assert!(
        elapsed >= floor,
        "terminated before the fixed bound: {elapsed:?}"
    );
    assert!(
        elapsed <= envelope,
        "outside the measured CI envelope: {elapsed:?}"
    );
    assert!(
        output.stdout.is_empty(),
        "no business receipt on watchdog stop: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "no diagnostics on watchdog stop: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Admin fixture client bound to the disposable test database (the admin
/// DSN's own database is not the fixture's schema home).
pub fn admin_db_client(admin: postgres::Config, runtime: &postgres::Config) -> Client {
    let mut admin = admin;
    admin.dbname(runtime.get_dbname().expect("runtime dbname"));
    admin
        .connect(postgres::NoTls)
        .expect("admin fixture connect")
}

/// libpq keyword/value escaping: quote a value and escape quote/backslash.
fn kw(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// Correctly escaped loopback keyword DSN from the authorized test Config,
/// rebound to a test-owned relay port. Only relay sessions select
/// sslmode=disable; production TLS semantics are unchanged.
pub fn relay_dsn(cfg: &postgres::Config, port: u16) -> String {
    let user = cfg.get_user().expect("runtime user").to_string();
    let password = String::from_utf8(cfg.get_password().expect("runtime password").to_vec())
        .expect("utf8 password");
    let dbname = cfg.get_dbname().expect("runtime dbname").to_string();
    format!(
        "host=127.0.0.1 port={port} user={} password={} dbname={} sslmode=disable",
        kw(&user),
        kw(&password),
        kw(&dbname)
    )
}

/// Test-owned ACCESS EXCLUSIVE lock held inside an open admin transaction.
/// The catalog check proves the fault is engaged before the CLI is launched;
/// Drop rolls the transaction back, releasing only this fixture's lock.
pub struct AccessExclusiveLock {
    admin: Client,
}

impl AccessExclusiveLock {
    pub fn hold(mut admin: Client, table: &str) -> Self {
        admin
            .batch_execute(&format!(
                "BEGIN; LOCK TABLE {table} IN ACCESS EXCLUSIVE MODE;"
            ))
            .unwrap();
        let held: i64 = admin
            .query_one(
                "SELECT count(*) FROM pg_locks WHERE relation=to_regclass($1) \
                 AND mode='AccessExclusiveLock' AND granted",
                &[&table],
            )
            .unwrap()
            .get(0);
        assert!(held > 0, "test-owned lock must be granted before launch");
        Self { admin }
    }
}

impl Drop for AccessExclusiveLock {
    fn drop(&mut self) {
        let _ = self.admin.batch_execute("ROLLBACK");
    }
}

/// Test-owned insert trigger executing pg_sleep for one engagement scope,
/// an explicit slow-statement fault for the server statement bound.
pub struct SleepFault {
    admin: Client,
    function: String,
    trigger: String,
}

impl SleepFault {
    pub fn install(mut admin: Client, engagement: &str, seconds: u32) -> Self {
        let function = "dwwait_sleep_fault".to_string();
        let trigger = "dwwait_sleep_fault".to_string();
        admin
            .batch_execute(&format!(
                "CREATE FUNCTION mission.{function}() RETURNS trigger LANGUAGE plpgsql AS \
                 $$ BEGIN PERFORM pg_sleep({seconds}); RETURN NEW; END $$; \
                 CREATE TRIGGER {trigger} AFTER INSERT ON mission.missions \
                 FOR EACH ROW WHEN (NEW.engagement_id='{engagement}'::uuid) \
                 EXECUTE FUNCTION mission.{function}();"
            ))
            .unwrap();
        let installed: i64 = admin
            .query_one(
                "SELECT count(*) FROM pg_trigger WHERE tgname=$1",
                &[&trigger],
            )
            .unwrap()
            .get(0);
        assert_eq!(installed, 1, "test-owned trigger must exist before launch");
        Self {
            admin,
            function,
            trigger,
        }
    }
}

impl Drop for SleepFault {
    fn drop(&mut self) {
        let _ = self.admin.batch_execute(&format!(
            "DROP TRIGGER {} ON mission.missions; DROP FUNCTION mission.{}();",
            self.trigger, self.function
        ));
    }
}

/// Scoped temp input file removed on drop.
pub struct InputFile(std::path::PathBuf);

impl InputFile {
    pub fn new(value: &serde_json::Value, tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("dw-wait-{tag}-{}.json", std::process::id()));
        std::fs::write(&path, value.to_string()).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for InputFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Test-owned deferrable constraint trigger raising a chosen ERRCODE at
/// COMMIT on mission.withdrawals for one engagement scope. Drop removes
/// only this fixture's trigger/function.
pub struct DeferredFault {
    admin: Client,
    function: String,
    trigger: String,
}

impl DeferredFault {
    pub fn install(mut admin: Client, engagement: &str, errcode: &str) -> Self {
        let function = format!("dwwait_commit_fault_{errcode}");
        let trigger = format!("dwwait_commit_fault_{errcode}");
        admin
            .batch_execute(&format!(
                "CREATE FUNCTION mission.{function}() RETURNS trigger LANGUAGE plpgsql AS \
                 $$ BEGIN RAISE EXCEPTION 'SYNTHETIC_SECRET_SENTINEL' USING ERRCODE='{errcode}'; END $$; \
                 CREATE CONSTRAINT TRIGGER {trigger} AFTER INSERT ON mission.withdrawals \
                 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW \
                 WHEN (NEW.engagement_id='{engagement}'::uuid) \
                 EXECUTE FUNCTION mission.{function}();"
            ))
            .unwrap();
        Self {
            admin,
            function,
            trigger,
        }
    }
}

impl Drop for DeferredFault {
    fn drop(&mut self) {
        let _ = self.admin.batch_execute(&format!(
            "DROP TRIGGER {} ON mission.withdrawals; DROP FUNCTION mission.{}();",
            self.trigger, self.function
        ));
    }
}
