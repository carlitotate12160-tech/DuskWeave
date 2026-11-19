use crate::https_support;
use crate::terrain::terrain_support::native;
use serde_json::{Value, json};
use uuid::Uuid;

fn epoch_for(eng: Uuid, cmp: Uuid) -> i64 {
    native::admin()
        .query_one(
            "SELECT epoch FROM execution.session_fences WHERE engagement_id=$1 AND campaign_id=$2",
            &[&eng, &cmp],
        )
        .unwrap()
        .get(0)
}

#[ignore]
#[test]
fn upgraded_0037_retains_legacy_reservation_guards_catalog_and_durability() {
    let _guard = native::qualified_existing_db();
    let q = https_support::registered_with(0xa01, |_| {});
    let m = &q["context"]["mission"];
    let eng = Uuid::parse_str(m["engagement_id"].as_str().unwrap()).unwrap();
    let cmp = Uuid::parse_str(m["campaign_id"].as_str().unwrap()).unwrap();
    let perm = Uuid::parse_str(m["permission_id"].as_str().unwrap()).unwrap();
    let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let mut admin = native::admin();

    let req = json!({
        "version": "m1_inference_reservation_request_v1",
        "engagement_id": eng, "campaign_id": cmp,
        "operator_ref": m["operator_ref"], "expected_mission_revision": 1,
        "permission_id": perm, "goal_ref": m["goal_ref"], "exercise_mode": m["exercise_mode"]
    });
    let absent_op = Uuid::new_v4();

    // 1. Absent 'recover' must return NULL/absent read-only without mutating session_fences.epoch.
    let epoch_before = epoch_for(eng, cmp);
    let rec_absent = broker
        .query_opt(
            "SELECT operation_id FROM execution.inference_reservation('recover',$1,$2,NULL) WHERE operation_id IS NOT NULL",
            &[&absent_op, &req],
        )
        .unwrap_or_else(|e| panic!("recover absent failed: {e}"));
    assert!(rec_absent.is_none());
    assert_eq!(epoch_for(eng, cmp), epoch_before);

    // 2. Null or unknown actions must refuse cleanly.
    for act in [None, Some("bogus"), Some("cancel_never_sent")] {
        let err = broker
            .query_one(
                "SELECT execution.inference_reservation($1,$2,$3,NULL)",
                &[&act, &absent_op, &req],
            )
            .unwrap_err();
        let msg = err.as_db_error().unwrap().message();
        assert!(
            msg == "inference_request_refused" || msg == "inference_request_invalid",
            "{msg}"
        );
    }

    // 3. Cancel requires an existing unclosed, never-released reservation.
    let cancel_missing = broker
        .query_one(
            "SELECT execution.inference_reservation('cancel',$1,$2,NULL)",
            &[&absent_op, &req],
        )
        .unwrap_err();
    assert_eq!(
        cancel_missing.as_db_error().unwrap().message(),
        "inference_missing_reservation"
    );

    // 4. Local synchronous_commit = off must refuse reservation inside a transaction.
    {
        let mut tx = broker.transaction().unwrap();
        tx.batch_execute("SET LOCAL synchronous_commit = off")
            .unwrap();
        let err = tx
            .query_one(
                "SELECT execution.inference_reservation('reserve',$1,$2,mission.current_inference_publication($3,$4,$1))",
                &[&absent_op, &req, &eng, &cmp],
            )
            .unwrap_err();
        assert_eq!(
            err.as_db_error().unwrap().message(),
            "inference_request_refused"
        );
    }

    // 5. All SECURITY DEFINER / search_path functions in the 0032 hardened set and 0037
    // must list pg_temp after pg_catalog and never reference unqualified pg_roles in prosrc.
    let bad_fns: Vec<String> = admin
        .query(
            "SELECT n.nspname || '.' || p.proname || ' cfg=' || coalesce(array_to_string(p.proconfig, ','), 'none') \
             FROM pg_catalog.pg_proc p JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace \
             WHERE ((n.nspname = 'execution' AND p.proname IN ( \
                 'bind_inference_input','close_fixture_session','close_inference','guard_mission_writer', \
                 'inference_call_state','inference_may_send','inference_reservation','inference_window_ms', \
                 'live_dns_result','live_effect_record','live_effect_reserve','live_effect_stop', \
                 'prepare_session','private_live_actor','private_live_control_scope','private_live_deadline', \
                 'private_live_mutation_guard','release_prepared_session','run_current','run_record', \
                 'run_start','withdraw_prepared_session','find_decision_run','https_loop_protected_held', \
                 'inference_capacity_snapshot','inference_profile_matches','bind_https_loop_input', \
                 'read_https_endpoint_map','protect_next_decision_capacity','convert_protected_capacity', \
                 'release_protected_capacity','run_phase_start','run_phase_complete','run_unknown','run_stop', \
                 'run_phase_diagnostic','genai_expected_intent','genai_binding_valid','https_loop_step_record', \
                 'https_loop_steps_for_run','https_loop_step_predecessor','record_https_loop_dns_intent', \
                 'record_https_loop_head_intent','record_https_loop_d4_outcome','run_https_loop_terminal', \
                 'https_loop_refresh_check','reserve_https_loop_dns','commit_https_loop_dns', \
                 'reserve_https_loop_head','commit_https_loop_head','block_https_loop_head', \
                 'mark_https_loop_effect_unknown','close_https_loop_effect_session','run_terminal', \
                 'run_summary_without_diagnostics' \
               )) OR (n.nspname = 'mission' AND p.proname IN ('live_stop_withdrawal')) \
               OR (n.nspname = 'terrain' AND p.proname IN ( \
                 'admit_dns_request','current_dns_context_view','accept_https_observation', \
                 'current_dns_endpoint_material','current_https_context_view','issued_dns_request' \
               )) OR (n.nspname = 'cognition' AND p.proname IN ('successor_reasoning_identity','reasoning_source_matches'))) \
               AND p.prosecdef \
               AND (p.proconfig IS NULL OR NOT EXISTS ( \
                   SELECT 1 FROM unnest(p.proconfig) cfg \
                   WHERE cfg LIKE 'search_path=pg_catalog, pg_temp%' OR cfg LIKE 'search_path=pg_catalog,pg_temp%' \
               ))",
            &[],
        )
        .unwrap()
        .into_iter()
        .map(|r| r.get(0))
        .collect();
    assert!(
        bad_fns.is_empty(),
        "unsafe search_path functions: {bad_fns:?}"
    );

    let unqual_roles: Vec<String> = admin
        .query(
            "SELECT n.nspname || '.' || p.proname \
             FROM pg_catalog.pg_proc p JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace \
             WHERE ((n.nspname = 'execution' AND p.proname IN ( \
                 'find_decision_run','https_loop_protected_held','inference_capacity_snapshot', \
                 'inference_profile_matches','inference_reservation','bind_https_loop_input', \
                 'read_https_endpoint_map','protect_next_decision_capacity','convert_protected_capacity', \
                 'release_protected_capacity','run_current','run_start','run_phase_start', \
                 'run_phase_complete','run_unknown','run_stop','run_phase_diagnostic','inference_window_ms', \
                 'genai_expected_intent','genai_binding_valid','assert_successor_pack','assert_autonomous_pack', \
                 'assert_current_pack_terrain','inference_may_send','https_loop_step_record', \
                 'https_loop_steps_for_run','https_loop_step_predecessor','record_https_loop_dns_intent', \
                 'record_https_loop_head_intent','record_https_loop_d4_outcome','run_https_loop_terminal', \
                 'https_loop_refresh_check','reserve_https_loop_dns','commit_https_loop_dns', \
                 'reserve_https_loop_head','commit_https_loop_head','block_https_loop_head', \
                 'mark_https_loop_effect_unknown','close_https_loop_effect_session','run_terminal', \
                 'run_summary_without_diagnostics' \
               )) OR (n.nspname = 'terrain' AND p.proname IN ( \
                 'accept_https_observation','current_dns_endpoint_material','current_https_context_view', \
                 'issued_dns_request','admit_dns_request' \
               )) OR (n.nspname = 'cognition' AND p.proname IN ('successor_reasoning_identity','reasoning_source_matches'))) \
               AND p.prosrc ~ '(^|[^.])\\mpg_roles\\M'",
            &[],
        )
        .unwrap()
        .into_iter()
        .map(|r| r.get(0))
        .collect();
    assert!(
        unqual_roles.is_empty(),
        "unqualified pg_roles in {unqual_roles:?}"
    );

    // 5b. Superuser principal and pg_temp.pg_roles shadow table cannot bypass 0035/0032 qualification.
    assert!(
        admin
            .query_one(
                "SELECT execution.inference_reservation('reserve',$1,$2,NULL)",
                &[&absent_op, &req],
            )
            .is_err(),
        "superuser principal must be rejected"
    );
    broker
        .batch_execute(
            "CREATE TEMP TABLE pg_roles (oid oid, rolname name, rolsuper bool, rolbypassrls bool, rolcreatedb bool, rolcreaterole bool); \
             INSERT INTO pg_roles VALUES (999999, session_user, false, false, false, false);",
        )
        .unwrap();
    let pg_temp_ok = broker
        .query_opt(
            "SELECT operation_id FROM execution.inference_reservation('recover',$1,$2,NULL) WHERE operation_id IS NOT NULL",
            &[&absent_op, &req],
        )
        .unwrap();
    assert!(pg_temp_ok.is_none());
    broker.batch_execute("DROP TABLE pg_temp.pg_roles").unwrap();

    // 6. Active session fence, missing native binding, wrong operator/goal/mode, duplicate recover/reserve preserve epoch,
    // and cancel works only when never released.
    let run_id = Uuid::parse_str(q["run_id"].as_str().unwrap()).unwrap();
    broker
        .execute("SELECT execution.run_start($1,$2)", &[&q, &"a".repeat(64)])
        .unwrap();
    let d1: Uuid = admin
        .query_one(
            "SELECT decision_one FROM execution.autonomous_runs WHERE run_id=$1",
            &[&run_id],
        )
        .unwrap()
        .get(0);
    // Reserve without native_input_bindings must fail.
    let no_bind = broker
        .query_one(
            "SELECT execution.inference_reservation('reserve',$1,$2,mission.current_inference_publication($3,$4,$1))",
            &[&d1, &req, &eng, &cmp],
        )
        .unwrap_err();
    assert!(
        no_bind.as_db_error().unwrap().message().contains("refused")
            || no_bind
                .as_db_error()
                .unwrap()
                .message()
                .contains("conflict")
    );
    // Duplicate 'recover'/'reserve' once reserved keeps epoch unchanged.
    let _s1: Value = broker
        .query_one("SELECT execution.https_loop_step_record($1,1)", &[&run_id])
        .unwrap()
        .get(0);
    let legacy_d = Uuid::new_v4();
    let mut q_leg =
        crate::run_support::registered_with(0xa02, crate::fixture::configure_permission);
    let now_leg: i64 = admin
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    q_leg["version"] = json!("m1_autonomous_run_request_v3");
    q_leg["run_profile"] = json!("autonomous-dns-fixture-v2");
    q_leg["inference_profile"] = json!(crate::fixture::FIXTURE_PROFILE);
    q_leg["deadline_at_ms"] = json!(now_leg * 1000 + 300_000);
    let lm = &q_leg["context"]["mission"];
    let leng = Uuid::parse_str(lm["engagement_id"].as_str().unwrap()).unwrap();
    let lcmp = Uuid::parse_str(lm["campaign_id"].as_str().unwrap()).unwrap();
    let lperm = Uuid::parse_str(lm["permission_id"].as_str().unwrap()).unwrap();
    let lreq = json!({
        "version": "m1_inference_reservation_request_v1",
        "engagement_id": leng, "campaign_id": lcmp,
        "operator_ref": lm["operator_ref"], "expected_mission_revision": 1,
        "permission_id": lperm, "goal_ref": lm["goal_ref"], "exercise_mode": lm["exercise_mode"]
    });
    let ld1: Uuid = {
        let mut r = q_leg.clone();
        r["run_id"] = json!(legacy_d);
        let _ = broker
            .execute("SELECT execution.run_start($1,$2)", &[&r, &"a".repeat(64)])
            .unwrap();
        broker
            .execute(
                "SELECT execution.run_phase_start($1,'decision_one')",
                &[&legacy_d],
            )
            .unwrap();
        broker
            .execute(
                "SELECT execution.run_phase_claim($1,'decision_one',$2)",
                &[&legacy_d, &"a".repeat(64)],
            )
            .unwrap();
        admin
            .query_one(
                "SELECT decision_one FROM execution.autonomous_runs WHERE run_id=$1",
                &[&legacy_d],
            )
            .unwrap()
            .get(0)
    };
    broker
        .execute(
            "SELECT execution.bind_inference_input($1,$2,$3,$4)",
            &[&leng, &lcmp, &ld1, &"b".repeat(64)],
        )
        .unwrap();
    for bad_k in ["operator_ref", "goal_ref", "exercise_mode"] {
        let mut bad = lreq.clone();
        bad[bad_k] = if bad_k == "exercise_mode" {
            json!("defender_informed")
        } else {
            json!(Uuid::new_v4())
        };
        assert!(broker.query_one(
            "SELECT execution.inference_reservation('reserve',$1,$2,mission.current_inference_publication($3,$4,$1))",
            &[&ld1, &bad, &leng, &lcmp],
        ).is_err());
    }
    let res1: Value = broker
        .query_one(
            "SELECT to_jsonb(execution.inference_reservation('reserve',$1,$2,mission.current_inference_publication($3,$4,$1)))",
            &[&ld1, &lreq, &leng, &lcmp],
        )
        .unwrap()
        .get(0);
    let ep_after_res = epoch_for(leng, lcmp);
    let rec1: Value = broker
        .query_one(
            "SELECT to_jsonb(execution.inference_reservation('recover',$1,$2,mission.current_inference_publication($3,$4,$1)))",
            &[&ld1, &lreq, &leng, &lcmp],
        )
        .unwrap()
        .get(0);
    assert_eq!(rec1["generation_call_id"], res1["generation_call_id"]);
    assert_eq!(epoch_for(leng, lcmp), ep_after_res);
    let dup1: Value = broker
        .query_one(
            "SELECT to_jsonb(execution.inference_reservation('reserve',$1,$2,mission.current_inference_publication($3,$4,$1)))",
            &[&ld1, &lreq, &leng, &lcmp],
        )
        .unwrap()
        .get(0);
    assert_eq!(dup1["generation_call_id"], res1["generation_call_id"]);
    assert_eq!(epoch_for(leng, lcmp), ep_after_res);

    // Cancel succeeds while never released, then idempotent recover keeps cancelled_never_sent.
    let can1: Value = broker
        .query_one(
            "SELECT to_jsonb(execution.inference_reservation('cancel',$1,$2,NULL))",
            &[&ld1, &lreq],
        )
        .unwrap()
        .get(0);
    assert_eq!(can1["kind"], "cancelled_never_sent");
    let rec_can: Value = broker
        .query_one(
            "SELECT to_jsonb(execution.inference_reservation('recover',$1,$2,NULL))",
            &[&ld1, &lreq],
        )
        .unwrap()
        .get(0);
    assert_eq!(rec_can["kind"], "cancelled_never_sent");
}
