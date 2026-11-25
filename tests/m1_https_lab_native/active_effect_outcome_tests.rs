#![cfg(test)]
//! Unprivileged active-HEAD post-fault and recovery outcome regressions.

use super::active_effect::{
    ExpectedFaultOutcome, validate_post_fault_state, validate_post_recover_sql,
    validate_recovery_receipts,
};
use super::active_effect_evidence::{ActiveHeadBinding, SqlActiveState};
use super::active_effect_tests::{sample_snapshot, sample_sql};
use serde_json::{Value, json};
use uuid::Uuid;

const UNK: ExpectedFaultOutcome = ExpectedFaultOutcome::ControlRootLossUnknown;
const STOP: ExpectedFaultOutcome = ExpectedFaultOutcome::Stopped;
type Mut<T> = fn(&mut T);

fn sample_receipt(
    run_id: Uuid,
    attempt: Uuid,
    status: &str,
    step3: &str,
    head_outcome: Value,
    disposition: Value,
) -> Value {
    json!({
        "run_id": run_id, "status": status, "live_qualified": true, "decision_count": 2,
        "dns_effect_count": 1, "head_effect_count": 0, "decision_history_count": 2,
        "terrain_history_count": 1, "release_sha256": "a".repeat(64),
        "steps": [
            { "step_index": 1, "status": "completed" },
            {
                "step_index": 2, "status": status, "head_attempt_id": attempt,
                "head_result_id": null, "head_outcome": head_outcome, "disposition": disposition
            },
            { "step_index": 3, "status": step3 }
        ]
    })
}

fn check_negative_post_fault_matrix(
    b: &ActiveHeadBinding,
    sql_unk: &SqlActiveState,
    snap_unk: &Value,
) {
    let cases: [(&str, Mut<SqlActiveState>, Mut<Value>); 5] = [
        (
            "completed_retained",
            |s| {
                s.run_status = "retained".into();
                s.step_status = "completed".into();
            },
            |v| {
                v["control"]["status"] = json!("retained");
                v["steps"][1]["status"] = json!("completed");
            },
        ),
        (
            "timeout_result",
            |s| s.head_result_present = true,
            |v| {
                v["steps"][1]["head_result_id"] = json!(Uuid::new_v4());
                v["steps"][1]["head_outcome"] = json!("timed_out");
            },
        ),
        (
            "result_recorded",
            |s| s.has_result_recorded_event = true,
            |v| {
                let att = v["sessions"][0]["attempt"].clone();
                v["events"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"attempt": att, "phase": "result_recorded"}));
            },
        ),
        (
            "substituted_attempt",
            |s| s.head_attempt_id = Some(Uuid::new_v4()),
            |v| v["steps"][1]["head_attempt_id"] = json!(Uuid::new_v4()),
        ),
        (
            "d3_executed",
            |s| s.d3_executed = true,
            |v| v["steps"][2]["status"] = json!("decision_running"),
        ),
    ];
    for (name, ms, mv) in cases {
        let (mut s, mut v) = (sql_unk.clone(), snap_unk.clone());
        ms(&mut s);
        mv(&mut v);
        assert!(
            validate_post_fault_state(UNK, b, &s, snap_unk, 2).is_err(),
            "{name} sql"
        );
        assert!(
            validate_post_fault_state(UNK, b, sql_unk, &v, 2).is_err(),
            "{name} snap"
        );
    }
    for outcome in [
        "head_response",
        "connection_failed",
        "tls_failed",
        "execution_refused",
        "timed_out",
        "blocked",
        "unknown_refresh",
    ] {
        let mut bad = snap_unk.clone();
        bad["steps"][1]["head_outcome"] = json!(outcome);
        assert!(
            validate_post_fault_state(UNK, b, sql_unk, &bad, 2).is_err(),
            "{outcome}"
        );
    }
    assert!(validate_post_fault_state(UNK, b, sql_unk, snap_unk, 3).is_err());
}

fn check_recovery_marker_preservation_and_rejections(
    exp: ExpectedFaultOutcome,
    b: &ActiveHeadBinding,
    status: &str,
    step3: &str,
    disp: Value,
) {
    let mut sql = sample_sql(b.run_id, b.attempt_id);
    (sql.run_status, sql.step_status) = (status.into(), status.into());
    for marker in [Value::Null, json!("unknown")] {
        let mut snap = sample_snapshot(b.run_id, b.attempt_id, status, step3);
        snap["steps"][1]["head_outcome"] = marker.clone();
        snap["steps"][1]["disposition"] = disp.clone();
        validate_post_fault_state(exp, b, &sql, &snap, 2).unwrap();

        let mut insp = sample_receipt(
            b.run_id,
            b.attempt_id,
            status,
            step3,
            marker.clone(),
            disp.clone(),
        );
        insp["decision_history_count"] = json!(1);
        let rec = sample_receipt(
            b.run_id,
            b.attempt_id,
            status,
            step3,
            marker.clone(),
            disp.clone(),
        );
        validate_recovery_receipts(exp, b, &snap, &insp, &rec, &rec).unwrap();
        validate_post_recover_sql(exp, b, &sql, &sql).unwrap();
    }
}

fn check_recovery_negative_controls(b: &ActiveHeadBinding, snap_stop: &Value, r_stop: &Value) {
    let mut erased = r_stop.clone();
    erased["steps"][1]["head_outcome"] = Value::Null;
    assert!(validate_recovery_receipts(STOP, b, snap_stop, r_stop, &erased, &erased).is_err());

    let mut changed_disp = r_stop.clone();
    changed_disp["steps"][1]["disposition"] = json!("other_reason");
    assert!(validate_recovery_receipts(STOP, b, snap_stop, &changed_disp, r_stop, r_stop).is_err());
    assert!(
        validate_recovery_receipts(STOP, b, snap_stop, r_stop, &changed_disp, &changed_disp)
            .is_err()
    );

    let mut diff_rec2 = r_stop.clone();
    diff_rec2["steps"][1]["disposition"] = json!("drifted");
    assert!(validate_recovery_receipts(STOP, b, snap_stop, r_stop, r_stop, &diff_rec2).is_err());

    let mut extra_head = r_stop.clone();
    extra_head["head_effect_count"] = json!(1);
    assert!(
        validate_recovery_receipts(STOP, b, snap_stop, r_stop, &extra_head, &extra_head).is_err()
    );

    let mut bad_outcome = r_stop.clone();
    bad_outcome["steps"][1]["head_outcome"] = json!("timed_out");
    assert!(
        validate_recovery_receipts(STOP, b, snap_stop, r_stop, &bad_outcome, &bad_outcome).is_err()
    );
}

#[test]
fn post_fault_and_recovery_validators_enforce_unknown_vs_stopped_without_replay() {
    let (run_id, attempt) = (Uuid::new_v4(), Uuid::new_v4());
    let b = ActiveHeadBinding {
        run_id,
        step_index: 2,
        attempt_id: attempt,
        session_generation: 2,
    };
    check_recovery_marker_preservation_and_rejections(
        UNK,
        &b,
        "unknown",
        "pending",
        json!("control_loss"),
    );
    check_recovery_marker_preservation_and_rejections(
        STOP,
        &b,
        "stopped",
        "stopped",
        json!("live_head_stopped"),
    );

    let mut sql_unk = sample_sql(run_id, attempt);
    (sql_unk.run_status, sql_unk.step_status) = ("unknown".into(), "unknown".into());
    let snap_unk = sample_snapshot(run_id, attempt, "unknown", "pending");
    let mut sql_stop = sample_sql(run_id, attempt);
    (sql_stop.run_status, sql_stop.step_status) = ("stopped".into(), "stopped".into());
    let mut snap_stop = sample_snapshot(run_id, attempt, "stopped", "stopped");
    snap_stop["steps"][1]["head_outcome"] = json!("unknown");
    snap_stop["steps"][1]["disposition"] = json!("live_head_stopped");
    assert!(validate_post_fault_state(UNK, &b, &sql_stop, &snap_stop, 2).is_err());
    assert!(validate_post_fault_state(STOP, &b, &sql_unk, &snap_unk, 2).is_err());

    check_negative_post_fault_matrix(&b, &sql_unk, &snap_unk);
    let r_stop = sample_receipt(
        run_id,
        attempt,
        "stopped",
        "stopped",
        json!("unknown"),
        json!("live_head_stopped"),
    );
    check_recovery_negative_controls(&b, &snap_stop, &r_stop);

    let mut sql_fake = sql_unk.clone();
    sql_fake.disposal_confirmed = true;
    assert!(validate_post_recover_sql(UNK, &b, &sql_unk, &sql_fake).is_err());
}
