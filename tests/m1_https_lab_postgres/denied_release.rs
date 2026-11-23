use crate::admission::{broker, db_err, now_ms, start_lab_run};
use crate::lab_support;
use crate::lineage::{commit_synthetic_step_decision, drive_step1_live_dns};
use crate::recovery::Renewals;
use crate::terrain::terrain_support::native;
use duskweave::m1_effect_control::authority::RestrictedWithdrawal;
use duskweave::m1_inference_profile::{HTTPS_LAB_HOSTNAME, HTTPS_LAB_IPV4, HTTPS_LAB_RUN_PROFILE};
use duskweave::m1_run::RunPhase;
use duskweave::m1_terrain_issued::IssuedDnsReference;
use duskweave::mission::{CampaignId, EngagementId, OperationId, OperatorRef};
use duskweave::postgres_m1_effect::PgStopWriter;
use duskweave::postgres_m1_terrain::PgTerrainStore;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_terrain_history::record_terrain_history;
use duskweave::withdrawal::{WithdrawalReason, WithdrawalRequest};
use postgres::Client;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

fn db_now_ms(c: &mut Client) -> i64 {
    c.query_one(
        "SELECT floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint",
        &[],
    )
    .unwrap()
    .get(0)
}

fn sleep_until_db_ms(c: &mut Client, target_ms: i64) {
    loop {
        let cur = db_now_ms(c);
        if cur >= target_ms {
            return;
        }
        let rem = (target_ms - cur).min(200) as u64;
        std::thread::sleep(Duration::from_millis(rem.max(10)));
    }
}

fn claim_cap_until_s(e: Uuid, c: Uuid) -> i64 {
    native::admin()
        .query_one(
            "SELECT least(cc.valid_until, cc.perceived_at + least(ev.lowest_ttl_seconds, 60))
             FROM terrain.current_claims cc
             JOIN terrain.evaluations ev
               ON (ev.engagement_id, ev.campaign_id, ev.operation_id) = (cc.engagement_id, cc.campaign_id, cc.operation_id)
             WHERE cc.engagement_id=$1 AND cc.campaign_id=$2",
            &[&e, &c],
        )
        .unwrap()
        .get(0)
}

fn setup_step2_head_ready(
    ttl_seconds: u32,
    dependency_refresh: &str,
) -> (
    Uuid,
    Uuid,
    Uuid,
    Value,
    duskweave::postgres_m1_run::PgRunStore,
    Renewals,
    (Uuid, i32),
) {
    let (e, c, run_id, reg, mut runs, rec) = start_lab_run(Uuid::new_v4());
    let renew = Renewals::start(run_id);
    let claim = drive_step1_live_dns(e, c, run_id, &reg, &mut runs, &rec, ttl_seconds);
    commit_synthetic_step_decision(
        &mut runs,
        &rec,
        &reg,
        2,
        json!({
            "category": "validate",
            "references": ["asset_0", "endpoint_0"],
            "manifest_reference": "https-head-v1",
            "dependency_refresh": dependency_refresh
        }),
    );
    runs.protect_next_decision(run_id, 2).unwrap();
    runs.start_phase(run_id, RunPhase::AcquisitionTwo).unwrap();
    runs.claim_phase(
        run_id,
        RunPhase::AcquisitionTwo,
        &lab_support::sample_release_sha(),
    )
    .unwrap();
    (e, c, run_id, reg, runs, renew, claim)
}

fn wait_advisory_blocked(pid: i32) {
    let mut admin = native::admin();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let blocked: bool = admin
            .query_one(
                "SELECT EXISTS(
                    SELECT 1 FROM pg_catalog.pg_locks
                    WHERE pid = $1 AND locktype = 'advisory' AND NOT granted
                )",
                &[&pid],
            )
            .unwrap()
            .get(0);
        if blocked {
            return;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    panic!("pid {pid} never blocked on campaign advisory lock");
}

fn timed_broker() -> (Client, i32) {
    let mut c = broker();
    c.batch_execute("SET statement_timeout = '5000'; SET lock_timeout = '5000';")
        .unwrap();
    let pid: i32 = c.query_one("SELECT pg_backend_pid()", &[]).unwrap().get(0);
    (c, pid)
}

fn timed_stop_writer() -> (Client, i32) {
    let mut c = native::client("DW_STOP_WRITER_DATABASE_URL");
    c.batch_execute("SET statement_timeout = '5000'; SET lock_timeout = '5000';")
        .unwrap();
    let pid: i32 = c.query_one("SELECT pg_backend_pid()", &[]).unwrap().get(0);
    (c, pid)
}

fn reserve_head(b: &mut Client, run_id: Uuid) -> (Uuid, i64, Value) {
    let rec: Value = b
        .query_one(
            "SELECT execution.live_https_loop_head_reserve($1,2)",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    let att = Uuid::parse_str(rec["request"]["attempt"].as_str().unwrap()).unwrap();
    let g = rec["request"]["session_generation"].as_i64().unwrap();
    (att, g, rec)
}

fn att_may_start(b: &mut Client, att: Uuid, g: i64) -> Result<i64, postgres::Error> {
    b.query_one(
        "SELECT execution.live_attempt_may_start($1,$2)",
        &[&att, &g],
    )
    .map(|r| r.get(0))
}

fn att_current(b: &mut Client, att: Uuid, g: i64) -> Result<(), postgres::Error> {
    b.query_one("SELECT execution.live_attempt_current($1,$2)", &[&att, &g])
        .map(|_| ())
}

fn att_phase(b: &mut Client, att: Uuid, g: i64, ph: &str) -> Result<(), postgres::Error> {
    b.query_one(
        "SELECT execution.live_attempt_head_phase($1,$2,$3)",
        &[&att, &g, &ph],
    )
    .map(|_| ())
}

fn reserve_head_err(b: &mut Client, run_id: Uuid) -> String {
    db_err(
        &b.query_one(
            "SELECT execution.live_https_loop_head_reserve($1,2)",
            &[&run_id],
        )
        .unwrap_err(),
    )
}

fn granted_phase(att: Uuid) -> String {
    native::admin()
        .query_one(
            "SELECT granted_phase FROM execution.live_effect_control WHERE attempt=$1",
            &[&att],
        )
        .unwrap()
        .get(0)
}

#[ignore]
#[test]
fn near_expiry_head_reserve_clamps_deadline_and_rejects_expired_start_and_renewal() {
    let _guard = native::qualified_existing_db();
    let (e, c, run_id, _reg, mut runs, renew, _claim) = setup_step2_head_ready(4, "none");
    let mut b = broker();
    let cap_until_ms = claim_cap_until_s(e, c) * 1000;
    // Wait until ~1.5s remain before cap_until so 5000ms attempt must clamp to cap_until_ms.
    sleep_until_db_ms(&mut b, cap_until_ms - 1800);

    let (att, g, head_rec) = reserve_head(&mut b, run_id);
    let dl = head_rec["request"]["deadline_at_ms"].as_i64().unwrap();
    let max_dur = head_rec["request"]["max_duration_ms"].as_i64().unwrap();
    let claim_until_s = head_rec["request"]["claim_valid_until_s"].as_i64().unwrap();
    assert!(dl <= claim_until_s * 1000 && dl <= cap_until_ms);
    assert!(max_dur > 0 && max_dur < 5000);
    sleep_until_db_ms(&mut b, cap_until_ms + 50);

    assert!(att_may_start(&mut b, att, g).is_err());
    assert!(att_current(&mut b, att, g).is_err());
    renew.stop();
    let _ = runs.stop(run_id);
}

#[ignore]
#[test]
fn expired_later_phase_grant_and_incompatible_bound_revision_refused() {
    let _guard = native::qualified_existing_db();
    // 1. Expired later phase grant ('tls') and renewal after may_start('connect').
    {
        let (e, c, run_id, _reg, mut runs, renew, _claim) = setup_step2_head_ready(4, "none");
        let mut b = broker();
        let cap_until_ms = claim_cap_until_s(e, c) * 1000;
        sleep_until_db_ms(&mut b, cap_until_ms - 1800);
        let (att, g, _) = reserve_head(&mut b, run_id);
        att_may_start(&mut b, att, g).unwrap();
        sleep_until_db_ms(&mut b, cap_until_ms + 50);
        assert!(att_phase(&mut b, att, g, "tls").is_err());
        assert!(att_current(&mut b, att, g).is_err());
        renew.stop();
        let _ = runs.stop(run_id);
    }

    // 2. Incompatible bound revision, integrity hold (before head reserve) & eval/qual invalidation (in session).
    {
        let (e, c, run_id, _reg, mut runs, renew, claim) = setup_step2_head_ready(30, "none");
        let (mut b, mut admin) = (broker(), native::admin());
        let op_id: Uuid = admin
            .query_one(
                "SELECT operation_id FROM terrain.current_claims WHERE claim_id = $1",
                &[&claim.0],
            )
            .unwrap()
            .get(0);
        admin
            .execute(
                "UPDATE terrain.current_claims SET claim_revision = $1 WHERE claim_id = $2",
                &[&(claim.1 + 1), &claim.0],
            )
            .unwrap();
        assert!(reserve_head_err(&mut b, run_id).contains("live_premise_ineligible"));
        admin
            .execute(
                "UPDATE terrain.current_claims SET claim_revision = $1 WHERE claim_id = $2",
                &[&claim.1, &claim.0],
            )
            .unwrap();
        admin.execute(
            "INSERT INTO terrain.integrity_conflicts (engagement_id, campaign_id, operation_id, claim_id, temporal_frontier, blocks_use, reason) VALUES ($1,$2,$3,$4,$5,true,'input_mismatch')",
            &[&e, &c, &op_id, &claim.0, &(now_ms() / 1000)],
        ).unwrap();
        assert!(reserve_head_err(&mut b, run_id).contains("live_premise_ineligible"));
        admin
            .execute(
                "DELETE FROM terrain.integrity_conflicts WHERE claim_id = $1",
                &[&claim.0],
            )
            .unwrap();

        let (att, g, head_rec) = reserve_head(&mut b, run_id);
        let qref =
            Uuid::parse_str(head_rec["request"]["qualification_ref"].as_str().unwrap()).unwrap();
        att_may_start(&mut b, att, g).unwrap();
        admin.execute("UPDATE terrain.evaluations SET outcome = 'ttl_zero_not_usable', lowest_ttl_seconds = 0 WHERE operation_id = $1", &[&op_id]).unwrap();
        assert!(att_current(&mut b, att, g).is_err());
        assert!(att_phase(&mut b, att, g, "tls").is_err());
        admin.execute("UPDATE terrain.evaluations SET outcome = 'qualified_dns_claim', lowest_ttl_seconds = 30 WHERE operation_id = $1", &[&op_id]).unwrap();
        admin.execute("UPDATE execution.live_qualifications SET valid_until_ms = $1 WHERE qualification_ref = $2", &[&(db_now_ms(&mut b) - 1), &qref]).unwrap();
        assert!(att_current(&mut b, att, g).is_err());
        assert!(att_phase(&mut b, att, g, "tls").is_err());
        renew.stop();
        let _ = runs.stop(run_id);
    }

    // 3. Refresh lineage: ttl=2s expires -> dns_refresh issues rev 2 with ttl=3s -> HEAD binds rev 2,
    //    succeeds on 'tls', then expires before 'head'.
    {
        let (e, c, run_id, _reg, mut runs, renew, _claim) =
            setup_step2_head_ready(2, "dns_once_if_stale");
        let mut b = broker();
        sleep_until_db_ms(&mut b, claim_cap_until_s(e, c) * 1000 + 100);
        let dns_ref: Value = b
            .query_one(
                "SELECT execution.live_https_loop_dns_reserve($1,2,'dns_refresh')",
                &[&run_id],
            )
            .unwrap()
            .get(0);
        let att_r = Uuid::parse_str(dns_ref["request"]["attempt"].as_str().unwrap()).unwrap();
        let g_r = dns_ref["request"]["session_generation"].as_i64().unwrap();
        let dl_r = att_may_start(&mut b, att_r, g_r).unwrap();
        let res_ok = json!({
            "schema": "m1_dns_ipc_v1", "kind": "RESULT", "attempt": att_r, "session_generation": g_r,
            "sequence": 3, "status": "completed", "observed_at_ms": dl_r - 2000, "rcode": 0,
            "truncated": false, "complete": true, "withheld_aliases": 0,
            "answers": [{"ipv4": HTTPS_LAB_IPV4, "ttl_seconds": 3}], "aliases": []
        });
        b.execute(
            "SELECT execution.live_attempt_dns_result($1,$2,$3)",
            &[&att_r, &g_r, &res_ok],
        )
        .unwrap();
        b.execute(
            "SELECT execution.live_attempt_stop($1,$2,true)",
            &[&att_r, &g_r],
        )
        .unwrap();
        let s2 = runs.https_step(run_id, 2).unwrap();
        let iref2 = IssuedDnsReference::live(
            EngagementId(e),
            CampaignId(c),
            s2.refresh_result_id.unwrap(),
            HTTPS_LAB_RUN_PROFILE,
        )
        .unwrap();
        let mut m_store = PgMissionStore::new(native::client("DW_TEST_DATABASE_URL"));
        let mut pts = PgTerrainStore::new(native::client("DW_TERRAIN_DATABASE_URL"));
        let (ev2, _, _, _) = pts.record_issued(&mut m_store, &iref2).unwrap().unwrap();
        record_terrain_history(&mut native::client("DW_TRAJECTORY_DATABASE_URL"), &ev2).unwrap();

        let (att_h, g_h, head_rec) = reserve_head(&mut b, run_id);
        assert_eq!(head_rec["request"]["parent_claim_revision"], json!(2));
        att_may_start(&mut b, att_h, g_h).unwrap();
        att_phase(&mut b, att_h, g_h, "tls").unwrap();
        sleep_until_db_ms(
            &mut b,
            head_rec["request"]["claim_valid_until_s"].as_i64().unwrap() * 1000 + 50,
        );
        assert!(att_phase(&mut b, att_h, g_h, "head").is_err());
        renew.stop();
        let _ = runs.stop(run_id);
    }
    let _ = HTTPS_LAB_HOSTNAME;
}

#[ignore]
#[test]
fn concurrent_stop_and_withdrawal_serialize_with_attempt_grants_and_inverse_ordering() {
    let _guard = native::qualified_existing_db();

    // 1. Committed run_stop prevents subsequent live_attempt_may_start under campaign advisory fence.
    {
        let (_e, _c, run_id, _reg, mut runs, renew, _claim) = setup_step2_head_ready(30, "none");
        let (att, g, _) = reserve_head(&mut broker(), run_id);
        renew.stop();
        runs.renew(run_id).unwrap();

        let (mut stop_conn, _) = timed_broker();
        let mut stop_tx = stop_conn.transaction().unwrap();
        stop_tx
            .execute("SELECT execution.run_stop($1)", &[&run_id])
            .unwrap();

        let (mut grant_conn, grant_pid) = timed_broker();
        let handle = std::thread::spawn(move || att_may_start(&mut grant_conn, att, g));
        wait_advisory_blocked(grant_pid);
        stop_tx.commit().unwrap();
        assert!(
            handle.join().unwrap().is_err(),
            "committed run_stop must prevent live_attempt_may_start"
        );
        let row = native::admin().query_one("SELECT phase, possibly_started FROM execution.live_effect_control WHERE attempt=$1", &[&att]).unwrap();
        assert_eq!(row.get::<_, String>(0), "reserved");
        assert!(!row.get::<_, bool>(1));
        let _ = runs.stop(run_id);
    }

    // 2. Committed mission.live_stop_withdrawal prevents subsequent live_attempt_head_phase('tls').
    {
        let (e, c, run_id, reg, mut runs, renew, _claim) = setup_step2_head_ready(30, "none");
        let op_ref = Uuid::parse_str(reg["operator_ref"].as_str().unwrap()).unwrap();
        let mut b = broker();
        let (att, g, _) = reserve_head(&mut b, run_id);
        att_may_start(&mut b, att, g).unwrap();
        renew.stop();
        runs.renew(run_id).unwrap();

        let (mut w_conn, _) = timed_stop_writer();
        let mut w_tx = w_conn.transaction().unwrap();
        let req = WithdrawalRequest {
            engagement_id: EngagementId(e),
            campaign_id: CampaignId(c),
            operator_ref: OperatorRef(op_ref),
            expected_mission_revision: 1,
            reason: WithdrawalReason::OperatorRequested,
        };
        let raw_q = serde_json::to_value(&req).unwrap();
        let w_op = Uuid::new_v4();
        w_tx.query_one(
            "SELECT mission.live_stop_withdrawal($1,$2,$3,$4)",
            &[&raw_q, &w_op, &Option::<i64>::None, &false],
        )
        .unwrap();

        let (mut phase_conn, phase_pid) = timed_broker();
        let handle = std::thread::spawn(move || att_phase(&mut phase_conn, att, g, "tls"));
        wait_advisory_blocked(phase_pid);
        w_tx.commit().unwrap();
        assert!(
            handle.join().unwrap().is_err(),
            "committed withdrawal must prevent subsequent tls phase grant"
        );
        assert_eq!(granted_phase(att), "connect");
        let _ = RestrictedWithdrawal::withdraw(
            &mut PgStopWriter::scoped(native::client("DW_STOP_WRITER_DATABASE_URL")),
            &req,
            OperationId(w_op),
            true,
        );
        let _ = runs.stop(run_id);
    }

    // 3. Inverse ordering: pre-STOP 'tls' grant commits before run_stop and is NOT mislabeled post-STOP.
    {
        let (_e, _c, run_id, _reg, mut runs, renew, _claim) = setup_step2_head_ready(30, "none");
        let mut b = broker();
        let (att, g, _) = reserve_head(&mut b, run_id);
        att_may_start(&mut b, att, g).unwrap();
        renew.stop();
        runs.renew(run_id).unwrap();

        let (mut grant_conn, _) = timed_broker();
        let mut grant_tx = grant_conn.transaction().unwrap();
        grant_tx
            .query_one(
                "SELECT execution.live_attempt_head_phase($1,$2,'tls')",
                &[&att, &g],
            )
            .unwrap();

        let (mut stop_conn, stop_pid) = timed_broker();
        let stop_handle = std::thread::spawn(move || {
            stop_conn.execute("SELECT execution.run_stop($1)", &[&run_id])
        });
        wait_advisory_blocked(stop_pid);
        grant_tx.commit().unwrap();
        stop_handle.join().unwrap().unwrap();
        assert_eq!(granted_phase(att), "tls");
        assert!(att_phase(&mut b, att, g, "head").is_err());
        let _ = runs.stop(run_id);
    }
}
