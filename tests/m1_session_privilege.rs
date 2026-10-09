//! Real-role qualification coverage for the shared database boundary: a Broker-class
//! login carrying any execution CREATE/ownership/mutation capability or prohibited
//! shared privilege must fail shared runtime qualification before a session mutation
//! through the actual CLI, while baseline actors still qualify. Admin is fixture control
//! only; cluster-wide grants serialize under db_support::db(), and every
//! probe restores exact original grants/owners and re-qualifies cleanly.

use duskweave::mission::{CampaignId, EngagementId, OperationId};
use duskweave::postgres_mission::qualify_runtime;
use serde_json::Value;
use std::process::Output;
use std::time::Duration;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;
#[path = "support/m1_session.rs"]
mod session_support;
#[path = "support/database_wait_db.rs"]
mod wait_db;
use session_support::*;

const PROBE: &str = "dw_m1_exec_probe";
const MID: &str = "dw_m1_set_mid";

fn run_cli(args: &[String], dsn: &str) -> Output {
    wait_db::run_bounded(&mut wait_db::cli(args, dsn), Duration::from_secs(15)).0
}

fn receipt(out: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(stdout.lines().next().unwrap_or(""))
        .unwrap_or_else(|e| panic!("receipt must be the first stdout line ({e}): {stdout}"))
}

fn assert_flags_off(v: &Value) {
    for flag in [
        "current_permission",
        "dispatch_granted",
        "acquisition_qualified",
        "host_control_qualified",
    ] {
        assert_eq!(v[flag], false, "{flag} must be explicit false: {v}");
    }
}

fn assert_no_leaks(out: &Output) {
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    for marker in ["postgresql", "password", "DSN"] {
        assert!(!all.contains(marker), "leaked marker {marker}: {all}");
    }
}

fn session_args(action: &str, op: OperationId, path: &str) -> Vec<String> {
    vec![
        "m1-session".into(),
        "--action".into(),
        action.into(),
        "--operation".into(),
        op.to_string(),
        "--input".into(),
        path.into(),
    ]
}

/// One over-privilege case: apply, prove qualification fails and the real
/// CLI refuses before any session mutation, restore, prove clean.
fn expect_unqualified(
    admin: &mut postgres::Client,
    grant: &str,
    revoke: &str,
    args: &[String],
    e: EngagementId,
    c: CampaignId,
    baseline_history: i64,
) {
    let pre_fence = fence(e, c);
    admin.batch_execute(grant).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert!(
            qualify_runtime(&mut priv_client()).is_err(),
            "qualification accepted over-privilege: {grant}"
        );
        let out = run_cli(args, &priv_dsn(db_port()));
        assert_eq!(
            out.status.code(),
            Some(1),
            "CLI must refuse before mutation: {grant}"
        );
        let r = receipt(&out);
        assert_eq!(r["outcome"], "refused", "{grant}");
        assert_flags_off(&r);
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("error=unqualified_runtime"),
            "qualification refusal must surface on stdout: {grant}"
        );
        assert_no_leaks(&out);
        assert_eq!(
            history_count(e, c),
            baseline_history,
            "refusal happened after a session mutation: {grant}"
        );
        assert_eq!(pre_fence, fence(e, c), "fence changed during refused operation: {grant}");
    }));
    admin.batch_execute(revoke).unwrap();
    assert!(
        qualify_runtime(&mut priv_client()).is_ok(),
        "restore left residual privilege: {revoke}"
    );
    if let Err(err) = result {
        std::panic::resume_unwind(err);
    }
}

/// Tests that logins granted prohibited database privileges (e.g. TRIGGER, UPDATE, DELETE,
/// TRUNCATE) on shared mission, trajectory, or execution tables are rejected during
/// runtime qualification before they can perform any mutations or effects.
#[test]
fn overprivileged_logins_fail_shared_qualification_before_mutation() {
    let _g = db();
    ensure_broker_logins();
    ensure_priv_login();
    let (e, c) = scope(0x7900);
    register_m1(e, c, 0);
    let op = session_op();
    let file = wait_db::InputFile::new(&request(e, c), &op.to_string());
    let args = session_args("prepare", op, file.path());
    let mut admin = admin_db_client();
    let owner: String = admin.query_one("SELECT current_user", &[]).unwrap().get(0);
    // Self-heal prior aborted probes: owner membership lets admin reclaim
    // execution objects, then every grant/owner returns to the fixture state.
    admin
        .batch_execute(&format!(
            "DO $$ BEGIN \
             IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='{PROBE}') THEN \
             CREATE ROLE {PROBE} NOLOGIN; END IF; \
             IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='{MID}') THEN \
             CREATE ROLE {MID} NOLOGIN; END IF; END $$; \
             GRANT {PROBE} TO {owner}; \
             GRANT {MID} TO {owner}; \
             ALTER SCHEMA execution OWNER TO {owner}; \
             ALTER TABLE execution.session_fences OWNER TO {owner}; \
             ALTER TABLE execution.session_history OWNER TO {owner}; \
             ALTER TABLE mission.withdrawals OWNER TO {owner}; \
             REVOKE {PROBE} FROM {owner}; \
             REVOKE {MID} FROM {owner}; \
             REVOKE {PROBE} FROM {PRIV_BROKER}; \
             REVOKE {MID} FROM {PRIV_BROKER}; \
             REVOKE {PROBE} FROM {MID}; \
             REVOKE CREATE ON SCHEMA execution FROM {PRIV_BROKER}; \
             REVOKE INSERT, UPDATE, DELETE, TRUNCATE, TRIGGER \
             ON execution.session_fences FROM {PRIV_BROKER}, {PROBE}, {MID}; \
             REVOKE INSERT, UPDATE, DELETE, TRUNCATE, TRIGGER \
             ON execution.session_history FROM {PRIV_BROKER}, {PROBE}, {MID}; \
             REVOKE TRIGGER ON mission.withdrawals FROM {PRIV_BROKER}, {PROBE}, {MID}; \
             REVOKE TRIGGER ON trajectory.withdrawal_history FROM {PRIV_BROKER}, {PROBE}, {MID}"
        ))
        .unwrap();
    // Baseline: the ordinary runtime login and the restricted probe Broker
    // qualify; the real CLI prepare claims the session.
    assert!(qualify_runtime(&mut runtime_client()).is_ok());
    assert!(qualify_runtime(&mut priv_client()).is_ok());
    let out = run_cli(&args, &priv_dsn(db_port()));
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let r = receipt(&out);
    assert_eq!(r["outcome"], "durable");
    assert_eq!(r["record"]["kind"], "prepared_no_effects");
    assert_flags_off(&r);
    let baseline_history = history_count(e, c);
    assert_eq!(baseline_history, 1);
    let p = PRIV_BROKER;
    for (grant, revoke) in [
        // Effective CREATE on the execution schema.
        (
            format!("GRANT CREATE ON SCHEMA execution TO {p}"),
            format!("REVOKE CREATE ON SCHEMA execution FROM {p}"),
        ),
        // Direct table privileges — INSERT/UPDATE/DELETE/TRUNCATE.
        (
            format!("GRANT INSERT ON execution.session_fences TO {p}"),
            format!("REVOKE INSERT ON execution.session_fences FROM {p}"),
        ),
        (
            format!("GRANT UPDATE ON execution.session_history TO {p}"),
            format!("REVOKE UPDATE ON execution.session_history FROM {p}"),
        ),
        (
            format!("GRANT DELETE ON execution.session_fences TO {p}"),
            format!("REVOKE DELETE ON execution.session_fences FROM {p}"),
        ),
        (
            format!("GRANT TRUNCATE ON execution.session_history TO {p}"),
            format!("REVOKE TRUNCATE ON execution.session_history FROM {p}"),
        ),
        // TRIGGER grants enable attaching a tampering/DoS trigger inside the
        // definer functions' own mutations — outside RLS entirely.
        (
            format!("GRANT TRIGGER ON execution.session_fences TO {p}"),
            format!("REVOKE TRIGGER ON execution.session_fences FROM {p}"),
        ),
        (
            format!("GRANT TRIGGER ON execution.session_history TO {p}"),
            format!("REVOKE TRIGGER ON execution.session_history FROM {p}"),
        ),
        // Inherited table privilege through a group role.
        (
            format!("GRANT UPDATE ON execution.session_fences TO {PROBE}; GRANT {PROBE} TO {p}"),
            format!(
                "REVOKE {PROBE} FROM {p}; REVOKE UPDATE ON execution.session_fences FROM {PROBE}"
            ),
        ),
        // Membership of the execution schema owner role.
        (
            format!("ALTER SCHEMA execution OWNER TO {PROBE}; GRANT {PROBE} TO {p}"),
            format!("REVOKE {PROBE} FROM {p}; ALTER SCHEMA execution OWNER TO {owner}"),
        ),
        // Direct ownership of a session table.
        (
            format!("ALTER TABLE execution.session_fences OWNER TO {p}"),
            format!("ALTER TABLE execution.session_fences OWNER TO {owner}"),
        ),
        // Membership of a session table's owner role.
        (
            format!("ALTER TABLE execution.session_history OWNER TO {PROBE}; GRANT {PROBE} TO {p}"),
            format!(
                "REVOKE {PROBE} FROM {p}; ALTER TABLE execution.session_history OWNER TO {owner}"
            ),
        ),
        // TRIGGER grants on shared mission and trajectory schemas.
        (
            format!("GRANT TRIGGER ON mission.withdrawals TO {p}"),
            format!("REVOKE TRIGGER ON mission.withdrawals FROM {p}"),
        ),
        (
            format!("GRANT TRIGGER ON trajectory.withdrawal_history TO {p}"),
            format!("REVOKE TRIGGER ON trajectory.withdrawal_history FROM {p}"),
        ),
        (
            format!("GRANT TRIGGER ON mission.withdrawals TO {PROBE}; GRANT {PROBE} TO {p}"),
            format!("REVOKE {PROBE} FROM {p}; REVOKE TRIGGER ON mission.withdrawals FROM {PROBE}"),
        ),
        (
            format!(
                "GRANT TRIGGER ON trajectory.withdrawal_history TO {PROBE}; GRANT {PROBE} TO {p}"
            ),
            format!(
                "REVOKE {PROBE} FROM {p}; REVOKE TRIGGER ON trajectory.withdrawal_history FROM {PROBE}"
            ),
        ),
        // INHERIT FALSE, SET TRUE TRIGGER role
        (
            format!("GRANT TRIGGER ON mission.withdrawals TO {PROBE}; GRANT {PROBE} TO {p} WITH INHERIT FALSE, SET TRUE"),
            format!("REVOKE {PROBE} FROM {p}; REVOKE TRIGGER ON mission.withdrawals FROM {PROBE}"),
        ),
        (
            format!("GRANT TRIGGER ON trajectory.withdrawal_history TO {PROBE}; GRANT {PROBE} TO {p} WITH INHERIT FALSE, SET TRUE"),
            format!("REVOKE {PROBE} FROM {p}; REVOKE TRIGGER ON trajectory.withdrawal_history FROM {PROBE}"),
        ),
        (
            format!("GRANT TRIGGER ON execution.session_fences TO {PROBE}; GRANT {PROBE} TO {p} WITH INHERIT FALSE, SET TRUE"),
            format!("REVOKE {PROBE} FROM {p}; REVOKE TRIGGER ON execution.session_fences FROM {PROBE}"),
        ),
        // A transitive SET TRUE chain to the TRIGGER role: refused.
        (
            format!("GRANT TRIGGER ON mission.withdrawals TO {PROBE}; GRANT {PROBE} TO {MID} WITH SET TRUE; GRANT {MID} TO {p} WITH SET TRUE"),
            format!("REVOKE {MID} FROM {p}; REVOKE {PROBE} FROM {MID}; REVOKE TRIGGER ON mission.withdrawals FROM {PROBE}"),
        ),
        // A SET-reachable owner of mission.withdrawals: refused; restore original owner.
        (
            format!("ALTER TABLE mission.withdrawals OWNER TO {PROBE}; GRANT {PROBE} TO {p} WITH SET TRUE"),
            format!("REVOKE {PROBE} FROM {p}; ALTER TABLE mission.withdrawals OWNER TO {owner}"),
        ),
    ] {
        expect_unqualified(&mut admin, &grant, &revoke, &args, e, c, baseline_history);
    }
    // Clean qualification afterward: the restored probe Broker replays its
    // own durable record through the real CLI.
    admin.batch_execute(&format!(
        "GRANT TRIGGER ON mission.withdrawals TO {PROBE}; \
         GRANT {PROBE} TO {MID} WITH INHERIT FALSE, SET FALSE; \
         GRANT {MID} TO {p} WITH INHERIT FALSE, SET TRUE"
    )).unwrap();
    let out = run_cli(&args, &priv_dsn(db_port()));
    assert!(
        out.status.success(),
        "{out_text}",
        out_text = String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(receipt(&out)["outcome"], "durable");
    assert_flags_off(&receipt(&out));
    assert_eq!(fence(e, c).unwrap().0, "prepared_no_effects");
    admin.batch_execute(&format!(
        "REVOKE {MID} FROM {p}; REVOKE {PROBE} FROM {MID}; REVOKE TRIGGER ON mission.withdrawals FROM {PROBE}"
    )).unwrap();
}
