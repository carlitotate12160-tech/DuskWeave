//! Schema-0030 receipt SQL regressions: qualification field bindings.
use super::fixture::*;
use serde_json::Value;
use uuid::Uuid;

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_receipt_binds_qualification_fields() {
    assert!(worker());
    let (e, c, _operator) = register_live(0x5d01, true);
    let (run, _, _) = fabricate_live_run(e, c, false, true);
    let qualified = |run: Uuid| -> bool {
        broker()
            .query_one(
                "SELECT execution.run_summary_without_diagnostics($1)",
                &[&run],
            )
            .unwrap()
            .get::<_, Value>(0)["live_qualified"]
            .as_bool()
            .unwrap()
    };
    assert!(
        qualified(run),
        "matching manifest qualification is live_qualified"
    );
    // Receipt comparison binds release, not merely the qualification ref.
    admin()
        .execute(
            "UPDATE execution.live_qualifications SET release_sha256=$1 WHERE run_profile=$2",
            &[&"f".repeat(64), &LAB],
        )
        .unwrap();
    assert!(!qualified(run), "release mismatch cannot be live_qualified");
    admin()
        .execute(
            "UPDATE execution.live_qualifications SET valid_until_ms=2 WHERE run_profile=$1",
            &[&LAB],
        )
        .unwrap();
    assert!(
        !qualified(run),
        "expired qualification cannot be live_qualified"
    );
}

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_pg_effect_store_rejects_unadmitted_run() {
    assert!(worker());
    use duskweave::m1_effect_session::EffectStore;
    use duskweave::postgres_m1_effect::PgEffectStore;
    use duskweave::Fail;

    let (e, c, _operator) = register_live(0x5d02, true);
    // fabricate_live_run(...,session=false,running=true) bypasses run_start:
    // context.mission is empty, the qualification does not match, and there is
    // no D1/closure/pack provenance. This run must never be admitted.
    let (run, _gen, _result_ref) = fabricate_live_run(e, c, false, true);

    let scoped_state = |run: Uuid| -> Value {
        let mut a = admin();
        let run_rows = |a: &mut postgres::Client, sql: &str| -> Value {
            a.query_one(sql, &[&run]).unwrap().get::<_, Value>(0)
        };
        let scope_rows = |a: &mut postgres::Client, sql: &str| -> Value {
            a.query_one(sql, &[&e, &c]).unwrap().get::<_, Value>(0)
        };
        serde_json::json!({
            "runs": run_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.autonomous_runs t WHERE t.run_id=$1"),
            "control": run_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.autonomous_control t WHERE t.run_id=$1"),
            "fence": scope_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.session_fences t WHERE t.engagement_id=$1 AND t.campaign_id=$2"),
            "sessions": run_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.live_effect_sessions t WHERE t.run_id=$1"),
            "effect_control": run_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.live_effect_control t WHERE t.run_id=$1"),
            "events": run_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.live_effect_events t WHERE t.run_id=$1"),
            "reservations": scope_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.inference_reservations t WHERE t.engagement_id=$1 AND t.campaign_id=$2"),
            "closures": scope_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.inference_closures t WHERE t.e=$1 AND t.c=$2"),
        })
    };
    let before = scoped_state(run);

    // The restricted broker function itself refuses this malformed request at
    // run_current with the public reason, before any adapter mapping.
    let error = broker()
        .query_one("SELECT execution.run_current($1)", &[&run])
        .unwrap_err();
    assert_eq!(db_message(&error), "run_authority_unavailable");

    // The adapter maps that refusal without inventing a session.
    let mut store = PgEffectStore::new(broker());
    assert_eq!(
        store.reserve(run).map(|_| ()),
        Err(Fail::State("live_effect_unavailable"))
    );

    // An exception alone is not proof: compare scoped run/control, session
    // fence, effect session/control/event and inference reservation/closure
    // rows. No new session, fence advance, charge or effect may be written.
    assert!(
        scoped_state(run) == before,
        "refused calls must not write scoped run/fence/effect or inference state"
    );
}
