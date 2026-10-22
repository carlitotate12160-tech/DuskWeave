//! Receipt-aware withdrawal, serialization rejection, and durable idempotency.
use super::{db_support, native, provider_permission, run_support};
use duskweave::m1_run::RunRecord;
use run_support::*;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

const WITHDRAW_RECEIPT_LIMIT: usize = 16384;
const WITHDRAW_ATTEMPTS: usize = 3;

#[test]
fn settlement_rejects_an_already_expired_deadline() {
    let end = Instant::now() - Duration::from_millis(1);
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--list")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    child.wait().unwrap();
    let reader = std::thread::spawn(Vec::<u8>::new);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        settle_withdraw(&mut child, reader, end)
    }));
    assert!(
        result.is_err(),
        "expired settlement accepted a terminal child"
    );
}

#[test]
fn settlement_rejects_a_collector_that_outlives_the_same_deadline() {
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--list")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    child.wait().unwrap();
    let reader = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(120));
        Vec::<u8>::new()
    });
    let end = Instant::now() + Duration::from_millis(20);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        settle_withdraw(&mut child, reader, end)
    }));
    assert!(
        result.is_err(),
        "collector extended the settlement deadline"
    );
}

struct Captured {
    status: std::process::ExitStatus,
    bytes: Vec<u8>,
}

/// One bounded withdrawal writer spawn: the same operation/request/input for
/// every attempt, a drained <=16KiB stdout receipt, and discarded stderr.
fn spawn_withdraw(
    input: &std::path::Path,
    root: &std::path::Path,
    operation: Uuid,
) -> (std::process::Child, std::thread::JoinHandle<Vec<u8>>) {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = command()
        .env("DW_M1_RUN_CONTROL_ROOT", root)
        .args([
            "withdraw",
            "--input",
            input.to_str().unwrap(),
            "--operation",
            &operation.to_string(),
            "--recover",
            "false",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pipe = child.stdout.take().expect("piped withdrawal stdout");
    let reader = std::thread::Builder::new()
        .name("withdraw_receipt".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe
                .take(WITHDRAW_RECEIPT_LIMIT as u64 + 1)
                .read_to_end(&mut bytes);
            bytes
        })
        .expect("withdraw receipt collector");
    (child, reader)
}

/// Child exit and receipt collection share the original fixture deadline.
/// Join only after collector completion, and never accept an expired result.
fn settle_withdraw(
    child: &mut std::process::Child,
    reader: std::thread::JoinHandle<Vec<u8>>,
    end: Instant,
) -> Captured {
    let status = loop {
        if Instant::now() >= end {
            let _ = child.kill();
            let _ = child.wait();
            panic!("withdrawal writer exceeded the shared settlement bound");
        }
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    while !reader.is_finished() {
        assert!(
            Instant::now() < end,
            "withdrawal collector exceeded the shared settlement bound"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let bytes = reader.join().expect("bounded receipt collector");
    assert!(
        Instant::now() < end,
        "withdrawal settlement expired before acceptance"
    );
    Captured { status, bytes }
}

fn terminal_error_line(line: &str) -> bool {
    line.strip_prefix("error=").is_some_and(|code| {
        !code.is_empty()
            && code
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    })
}

/// The `cmd_withdraw` producer emits exactly one JSON receipt line, optionally
/// followed by one terminal `error=<category>` line on failure. Anything else
/// â€” two JSON lines, a leading error, trailing junk, invalid UTF-8, or an
/// oversized capture â€” is an unclassifiable receipt, not a retryable result.
fn withdraw_receipt(bytes: &[u8]) -> Option<Value> {
    if bytes.len() > WITHDRAW_RECEIPT_LIMIT {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let mut receipt = None;
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with('{') && receipt.is_none() {
            receipt = Some(serde_json::from_str::<Value>(line).ok()?);
            continue;
        }
        let terminal = receipt.is_some() && lines.peek().is_none() && terminal_error_line(line);
        if !terminal && !line.trim().is_empty() {
            return None;
        }
    }
    receipt.filter(|r| r.is_object())
}

enum WithdrawOutcome {
    Durable,
    Retry,
    Failed(&'static str),
}

fn disposition_name(outcome: &WithdrawOutcome) -> &'static str {
    match outcome {
        WithdrawOutcome::Durable => "durable",
        WithdrawOutcome::Retry => "serialization_retry",
        WithdrawOutcome::Failed(reason) => reason,
    }
}

/// Only a failed exit whose receipt names the same operation with a known
/// serialization rejection may resubmit. Unknown outcomes, storage errors,
/// conflicts, foreign receipts and unreadable output are terminal failures.
fn classify_withdraw(success: bool, receipt: Option<&Value>, operation: Uuid) -> WithdrawOutcome {
    let retryable = !success
        && receipt.is_some_and(|r| {
            r["result"] == "rejected"
                && r["reason"] == "serialization_retry"
                && r["operation"] == json!(operation)
        });
    if retryable {
        return WithdrawOutcome::Retry;
    }
    if success && receipt.is_some_and(|r| r["result"] == "durable") {
        return WithdrawOutcome::Durable;
    }
    WithdrawOutcome::Failed(if success {
        "success_without_durable_receipt"
    } else if receipt.is_none() {
        "unreadable_or_missing_receipt"
    } else {
        "non_retryable_rejection"
    })
}

/// The blocked backend must be this scope's Mission writer waiting on its
/// advisory fence inside `INSERT INTO mission.withdrawals`, not any backend.
fn withdraw_backend_blocked(locker: &mut postgres::Client, key: &str, writer: &str) -> bool {
    locker
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM pg_locks l JOIN pg_stat_activity a ON a.pid=l.pid \
             WHERE l.locktype='advisory' AND NOT l.granted AND l.objsubid=1 \
             AND a.datname=current_database() AND a.usename=$2 \
             AND (l.classid::bigint<<32|l.objid::bigint)::bigint=hashtextextended($1,0))",
            &[&key, &writer],
        )
        .unwrap()
        .get(0)
}

fn assert_durable_withdrawal(
    receipt: &Value,
    rec: &RunRecord,
    operation: Uuid,
    marker: &std::path::Path,
) -> Value {
    let m = &rec.request.context.mission;
    assert_eq!(receipt["authority_state"], "withdrawn");
    assert_eq!(receipt["history_source"], "trajectory");
    assert_eq!(receipt["history"], "completed");
    assert_eq!(receipt["complete_history"], true);
    assert_eq!(receipt["current_permission"], false);
    assert_eq!(receipt["continuation_blocked"], true);
    let contract = &receipt["contract"];
    assert_eq!(contract["operation_id"], json!(operation));
    assert_eq!(contract["request"]["engagement_id"], json!(m.engagement_id));
    assert_eq!(contract["request"]["campaign_id"], json!(m.campaign_id));
    assert_eq!(contract["kind"], "mission_authority_withdrawn");
    assert!(
        marker.exists(),
        "a terminal marker must not be cleared to resume"
    );
    assert_eq!(std::fs::read(marker).unwrap(), b"STOP\n");
    assert_eq!(
        db_support::count("mission.withdrawals", m.engagement_id, m.campaign_id),
        1,
        "exactly one durable Mission withdrawal"
    );
    let history: i64 = native::admin()
        .query_one(
            "SELECT count(*) FROM trajectory.withdrawal_history \
             WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3 \
             AND event_id=$4 AND status='accepted' AND contract=$5",
            &[
                &m.engagement_id.0,
                &m.campaign_id.0,
                &operation,
                &Uuid::parse_str(contract["event_id"].as_str().unwrap()).unwrap(),
                contract,
            ],
        )
        .unwrap()
        .get(0);
    assert_eq!(
        history, 1,
        "one accepted Trajectory history bound to the durable event"
    );
    let summary = broker_store().summary(rec.request.run_id).unwrap();
    assert_eq!(summary["effect_count"], 0);
    assert_eq!(summary["inference"], json!([]));
    contract.clone()
}

/// Real CLI -> local campaign STOP -> held SQL fence -> restricted Mission
/// withdrawal -> Trajectory history. `bump_epoch` performs the controlled
/// fault injection: one administered `session_fences.epoch` update commits
/// while the writer's INSERT is fenced, forcing its own epoch touch to abort
/// with a known serialization rejection before bounded resubmission.
pub(super) fn signal_before_sql(slot: u128, bump_epoch: bool) {
    let q = registered_with(slot, provider_permission);
    let rec = start_only(&q);
    let custodian = LeaseCustodian::start(&rec, broker_store());
    let root = control_root();
    let m = &rec.request.context.mission;
    let operation = consent_id();
    let doc = json!({"engagement_id":m.engagement_id,"campaign_id":m.campaign_id,
        "operator_ref":m.operator_ref,"expected_mission_revision":1,"reason":"operator_requested"});
    let input = std::env::temp_dir().join(format!("dw-pre-sql-{}.json", rec.request.run_id));
    std::fs::write(&input, serde_json::to_vec(&doc).unwrap()).unwrap();
    let marker = root.join(format!(
        "{}.{}.campaign-stop",
        m.engagement_id.0, m.campaign_id.0
    ));
    let writer: String = native::client("DW_TEST_DATABASE_URL")
        .query_one("SELECT current_user", &[])
        .unwrap()
        .get(0);
    let key = scope_key(&rec);
    let mut locker = native::admin();
    lock_scope(&mut locker, &key);
    let (mut child, mut reader) = spawn_withdraw(&input, &root, operation);
    let signal_deadline = Instant::now() + Duration::from_secs(5);
    while !marker.exists() {
        assert!(
            Instant::now() < signal_deadline,
            "missing pre-SQL signal for withdraw"
        );
        assert!(
            child.try_wait().unwrap().is_none(),
            "withdrawal exited before signal"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(std::fs::metadata(&marker).unwrap().len() <= 5);
    while !withdraw_backend_blocked(&mut locker, &key, &writer) {
        assert!(
            Instant::now() < signal_deadline,
            "withdrawal never reached the held SQL fence"
        );
        assert!(
            child.try_wait().unwrap().is_none(),
            "withdrawal refused before held fence"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    let status: String = locker
        .query_one(
            "SELECT status FROM execution.autonomous_control WHERE run_id=$1",
            &[&rec.request.run_id],
        )
        .unwrap()
        .get(0);
    assert_eq!(
        status, "running",
        "a local intent is not a committed SQL withdrawal"
    );
    if bump_epoch {
        locker
            .execute(
                "UPDATE execution.session_fences SET epoch=epoch+1 \
                 WHERE engagement_id=$1 AND campaign_id=$2",
                &[&m.engagement_id.0, &m.campaign_id.0],
            )
            .unwrap();
    }
    unlock_scope(&mut locker, &key);
    let end = Instant::now() + Duration::from_secs(8);
    let mut attempt = 0usize;
    let (receipt, outcome) = loop {
        attempt += 1;
        let captured = settle_withdraw(&mut child, reader, end);
        let receipt = withdraw_receipt(&captured.bytes);
        let outcome = classify_withdraw(captured.status.success(), receipt.as_ref(), operation);
        println!(
            "withdraw attempt={attempt} exit={:?} receipt_bytes={} receipt_limit={} disposition={}",
            captured.status.code(),
            captured.bytes.len(),
            WITHDRAW_RECEIPT_LIMIT,
            disposition_name(&outcome)
        );
        match outcome {
            WithdrawOutcome::Retry if attempt < WITHDRAW_ATTEMPTS && Instant::now() < end => {
                let next = spawn_withdraw(&input, &root, operation);
                child = next.0;
                reader = next.1;
            }
            _ => break (receipt, outcome),
        }
    };
    assert!(
        matches!(outcome, WithdrawOutcome::Durable),
        "withdrawal did not reach a durable receipt within {WITHDRAW_ATTEMPTS} bounded attempts"
    );
    assert!(
        attempt >= 2 || !bump_epoch,
        "epoch-bump injection did not serialize the first attempt"
    );
    let receipt = receipt.expect("durable outcome implies a parsed receipt");
    let contract = assert_durable_withdrawal(&receipt, &rec, operation, &marker);
    let (mut replay, replay_reader) = spawn_withdraw(&input, &root, operation);
    let replay = settle_withdraw(
        &mut replay,
        replay_reader,
        Instant::now() + Duration::from_secs(8),
    );
    assert!(replay.status.success(), "identical resubmission fails");
    let replay_receipt = withdraw_receipt(&replay.bytes).expect("replay receipt");
    assert_eq!(replay_receipt["result"], "durable");
    assert_eq!(
        replay_receipt["contract"]["event_id"], contract["event_id"],
        "replay returns the original event, never a second withdrawal"
    );
    assert_eq!(
        db_support::count("mission.withdrawals", m.engagement_id, m.campaign_id),
        1,
        "identical replay cannot append a second withdrawal"
    );
    assert_eq!(
        db_support::count_where(
            "trajectory.withdrawal_history",
            "AND status='accepted'",
            m.engagement_id,
            m.campaign_id
        ),
        1,
        "identical replay cannot append a second history"
    );
    let _ = custodian.finish();
    std::fs::remove_file(&input).unwrap();
}

pub(super) fn fenced_epoch_bump_serializes_first_attempt_then_bounded_resubmission_is_durable_once()
{
    let _guard = native::db();
    refresh();
    signal_before_sql(0xc944, true);
}

pub(super) fn withdrawal_receipt_parser_bounds_shape_and_size() {
    let durable = b"{\"result\":\"durable\",\"contract\":{\"operation_id\":\"x\"}}\n";
    assert!(withdraw_receipt(durable).is_some());
    let rejected = b"{\"result\":\"rejected\"}\nerror=serialization_retry\n";
    assert!(withdraw_receipt(rejected).is_some());
    let oversized = vec![b' '; WITHDRAW_RECEIPT_LIMIT + 1];
    for bad in [
        &b"garbage line\n{\"result\":\"durable\"}\n"[..],
        &b"{\"result\":\"rejected\"}\n{\"result\":\"durable\"}\n"[..],
        &b"error=serialization_retry\n{\"result\":\"rejected\"}\n"[..],
        &b"{\"result\":\"rejected\"}\nerror=serialization_retry\ntrailing\n"[..],
        &b"{\"result\":\"rejected\"}\nerror=bad code\n"[..],
        &b"{not json}\n"[..],
        &b""[..],
        &oversized[..],
        &[0xff, 0xfe][..],
    ] {
        assert!(
            withdraw_receipt(bad).is_none(),
            "malformed receipt accepted"
        );
    }
}

pub(super) fn withdrawal_retry_classifier_accepts_only_bounded_serialization_rejection() {
    let op = Uuid::from_u128(0xbeef);
    let retryable = json!({"result":"rejected","reason":"serialization_retry","operation":op});
    assert!(matches!(
        classify_withdraw(false, Some(&retryable), op),
        WithdrawOutcome::Retry
    ));
    for rejected in [
        json!({"result":"unknown","reason":"commit_unknown","operation":op}),
        json!({"result":"rejected","reason":"commit_unknown","operation":op}),
        json!({"result":"rejected","reason":"storage_error","operation":op}),
        json!({"result":"rejected","reason":"integrity_conflict","operation":op}),
        json!({"result":"rejected","reason":"serialization_retry","operation":Uuid::from_u128(1)}),
        json!({"result":"rejected","reason":"serialization_retry"}),
        json!({"result":"not_committed","operation":op}),
    ] {
        assert!(
            matches!(
                classify_withdraw(false, Some(&rejected), op),
                WithdrawOutcome::Failed(_)
            ),
            "non-retryable receipt resubmits: {rejected}"
        );
    }
    assert!(matches!(
        classify_withdraw(false, None, op),
        WithdrawOutcome::Failed(_)
    ));
    let durable = json!({"result":"durable"});
    assert!(matches!(
        classify_withdraw(true, Some(&durable), op),
        WithdrawOutcome::Durable
    ));
    for receipt in [
        Some(&retryable),
        Some(&json!({"result":"not_committed","operation":op})),
        None,
    ] {
        assert!(
            matches!(
                classify_withdraw(true, receipt, op),
                WithdrawOutcome::Failed(_)
            ),
            "successful exit without a durable receipt passes as durable"
        );
    }
}
