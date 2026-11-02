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
fn sql_live_pg_effect_store_adapter_paths() {
    assert!(worker());
    use duskweave::m1_effect_session::EffectStore;
    use duskweave::postgres_m1_effect::PgEffectStore;

    let (e, c, _operator) = register_live(0x5d02, true);
    let (run, _gen, _result_ref) = fabricate_live_run(e, c, false, true);

    let mut store = PgEffectStore::new(broker());

    let session = store.reserve(run).expect("reserve session in store");
    assert_eq!(session.run_id, run);

    let inspected = store.inspect(run).expect("inspect session in store");
    assert_eq!(inspected.run_id, run);

    let dl1 = store.may_start(&session).expect("may_start deadline");
    let dl2 = store.current_deadline(&session).expect("current deadline");
    assert_eq!(dl1, dl2);

    store.stop(&session).expect("stop session");
    store
        .close_disposed(&session)
        .expect("close disposed session");
}
