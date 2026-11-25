use crate::admission::{
    assert_42501, base_registration_json, broker, now_ms, register_lab_campaign, seed_lab_qual,
    seed_qual, start_lab_run,
};
use crate::episode::{CliRun, ControlDir, run_decision_phase};
use crate::lab_support;
use crate::recovery::Renewals;
use crate::terrain::terrain_support::native;
use duskweave::m1_https::{
    HttpsHeadResult, HttpsOutcomeKind, HttpsSourceKind, RedirectDisposition, VERSION_HEAD_RESULT,
};
use duskweave::m1_inference_profile::{
    FIXTURE_PRO_V2_PROFILE, HTTPS_LAB_RESOLVER, LIVE_RUN_PROFILE,
};
use duskweave::m1_run::{RunPhase, RunRequest};
use duskweave::m1_terrain_input::DnsAnswer;
use duskweave::m1_terrain_issued::IssuedDnsReference;
use duskweave::mission::{CampaignId, EngagementId, OperationId};
use duskweave::postgres_m1_effect::PgEffectStore;
use duskweave::postgres_m1_run::PgRunStore;
use duskweave::postgres_m1_terrain::{PgTerrainStore, https::PgHttpsTerrainStore};
use duskweave::postgres_mission::PgMissionStore;
use duskweave::postgres_terrain_history::{record_https_terrain_history, record_terrain_history};
use serde_json::json;
use uuid::Uuid;

pub(crate) fn drive_fixture_scoped_d1_d2(pre_start_s: i64) -> (CliRun, ControlDir, Renewals) {
    let rel = lab_support::sha256_hex(&std::fs::read(env!("CARGO_BIN_EXE_duskweave")).unwrap());
    let (e, c, run_id) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let mut reg = base_registration_json(e, c, Uuid::new_v4(), &rel);
    let ip = &mut reg["m1_inference_permission"];
    (
        ip["version"],
        ip["run_profile"],
        ip["execution_mode"],
        ip["profile"],
    ) = (
        json!("m1_https_loop_v1"),
        json!("m1_https_loop_v1"),
        json!("controlled_fixture"),
        json!(FIXTURE_PRO_V2_PROFILE),
    );
    ip.as_object_mut().unwrap().remove("live_binding");
    let perm_id = ip["permission_id"].clone();
    register_lab_campaign(&reg).unwrap();
    let req_v = json!({
        "version": "m1_https_loop_v1", "run_id": run_id, "run_profile": "m1_https_loop_v1",
        "execution_mode": "controlled_fixture", "inference_profile": FIXTURE_PRO_V2_PROFILE, "deadline_at_ms": now_ms() + 600_000,
        "context": { "version": "m1_context_prepare_request_v1", "mission": { "version": "m1_inference_reservation_request_v1", "engagement_id": e, "campaign_id": c, "operator_ref": reg["operator_ref"], "expected_mission_revision": 1, "permission_id": perm_id, "goal_ref": reg["goal_ref"], "exercise_mode": "blind" }, "candidates": [{"asset_ref": reg["included_assets"][0], "priority": "high"}] },
        "seed_names": [{"asset_ref": reg["included_assets"][0], "name": "head.duskweave.test"}]
    });
    let req = RunRequest::parse(&serde_json::to_vec(&req_v).unwrap()).unwrap();
    let mut runs = PgRunStore::new(broker());
    assert!(runs.start(&req, &rel).unwrap());
    let rec = runs.inspect(run_id).unwrap();
    let (ctrl, renewals) = (ControlDir::new(), Renewals::start(run_id));
    let (mut ms, mut pts) = (
        PgMissionStore::new(native::client("DW_TEST_DATABASE_URL")),
        PgTerrainStore::new(native::client("DW_TERRAIN_DATABASE_URL")),
    );
    for (step, dp, ap) in [
        (1, RunPhase::DecisionOne, RunPhase::AcquisitionOne),
        (2, RunPhase::DecisionTwo, RunPhase::AcquisitionTwo),
    ] {
        run_decision_phase(&mut runs, run_id, dp, &ctrl);
        runs.resolve_https_step_action(&rec, step).unwrap();
        runs.protect_next_decision(run_id, step).unwrap();
        runs.start_phase(run_id, ap).unwrap();
        runs.claim_phase(run_id, ap, &rel).unwrap();
        if step == 1 {
            let mut dr = runs.reserve_https_dns(run_id, 1, "initial", 0).unwrap();
            dr.answers = vec![DnsAnswer {
                ipv4: "198.51.100.10".into(),
                ttl_seconds: 30,
            }];
            runs.commit_https_dns(run_id, 1, "initial", &dr).unwrap();
            let iref = IssuedDnsReference::controlled_fixture(
                EngagementId(e),
                CampaignId(c),
                dr.correlation_id,
                "m1_https_loop_v1",
            )
            .unwrap();
            let (ev, _, _, _) = pts.record_issued(&mut ms, &iref).unwrap().unwrap();
            record_terrain_history(&mut native::client("DW_TRAJECTORY_DATABASE_URL"), &ev).unwrap();
        } else {
            let hr = runs.reserve_https_head(run_id, 2).unwrap();
            let res = HttpsHeadResult {
                version: VERSION_HEAD_RESULT.into(),
                engagement_id: hr.engagement_id,
                campaign_id: hr.campaign_id,
                run_id: hr.run_id,
                decision_id: hr.decision_id,
                attempt_id: hr.attempt_id,
                result_id: Uuid::new_v4(),
                asset_ref: hr.asset_ref,
                endpoint_ref: hr.endpoint_ref,
                hostname: hr.hostname,
                ipv4: hr.ipv4,
                port: 443,
                approved_path: hr.approved_path,
                method: "HEAD".into(),
                parent_claim_id: hr.parent_claim_id,
                parent_claim_revision: hr.parent_claim_revision,
                perceived_at: pre_start_s,
                uncertainty_seconds: 1,
                outcome: HttpsOutcomeKind::ConnectionFailed,
                status_code: None,
                redirect: RedirectDisposition::None,
                source_kind: HttpsSourceKind::FixtureHttpsAcquisition,
                limitations: vec!["single_endpoint_only".into()],
            };
            runs.commit_https_head(run_id, 2, &res).unwrap();
            let op = OperationId(
                runs.https_step(run_id, 2)
                    .unwrap()
                    .head_terrain_operation_id
                    .unwrap(),
            );
            let ev = PgHttpsTerrainStore::new(&mut pts)
                .record_https(&mut ms, &res, op)
                .unwrap();
            record_https_terrain_history(&mut native::client("DW_TRAJECTORY_DATABASE_URL"), &ev)
                .unwrap();
        }
        runs.complete_phase(run_id, ap).unwrap();
    }
    ((e, c, run_id, reg, runs, rec, rel), ctrl, renewals)
}

fn extract_fn_body(sql: &str, name: &str) -> String {
    let normalized = sql.replace("\r\n", "\n");
    let header = format!("CREATE OR REPLACE FUNCTION {name}(");
    let start = normalized
        .find(&header)
        .unwrap_or_else(|| panic!("missing {name} in migration"));
    let marker = normalized[start..]
        .find("AS $$")
        .unwrap_or_else(|| panic!("missing AS $$ for {name}"))
        + start
        + 5;
    let end = normalized[marker..]
        .find("$$;")
        .unwrap_or_else(|| panic!("missing $$; for {name}"))
        + marker;
    normalized[marker..end].to_owned()
}

#[ignore]
#[test]
fn schema0038_catalog_security_definer_and_attempt_key_constraints() {
    let _guard = native::qualified_existing_db();
    let mut a = native::admin();
    let expected_procs = [
        ("execution", "assert_current_pack_terrain"),
        ("execution", "live_https_loop_dns_reserve"),
        ("execution", "live_https_loop_head_reserve"),
        ("execution", "live_attempt_current"),
        ("execution", "live_attempt_may_start"),
        ("execution", "live_attempt_head_phase"),
        ("execution", "live_attempt_dns_result"),
        ("execution", "live_attempt_head_result"),
        ("execution", "live_attempt_stop"),
        ("execution", "read_https_loop_head_result"),
        ("terrain", "issued_dns_request"),
        ("terrain", "accept_https_observation"),
    ];
    for (ns, name) in expected_procs {
        let rows = a
            .query(
                "SELECT p.prosecdef, coalesce(p.proconfig, ARRAY[]::text[]),
                        has_function_privilege('public', p.oid, 'EXECUTE')
                 FROM pg_catalog.pg_proc p
                 JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
                 WHERE n.nspname = $1 AND p.proname = $2",
                &[&ns, &name],
            )
            .unwrap_or_else(|e| panic!("missing {ns}.{name}: {e}"));
        assert!(!rows.is_empty(), "missing {ns}.{name}");
        for row in rows {
            let secdef: bool = row.get(0);
            let cfg: Vec<String> = row.get(1);
            let pub_exec: bool = row.get(2);
            assert!(secdef, "{ns}.{name} must be SECURITY DEFINER");
            assert!(
                cfg.iter().any(|c| c == "search_path=pg_catalog, pg_temp"),
                "{ns}.{name} must pin search_path=pg_catalog, pg_temp, got {cfg:?}"
            );
            assert!(!pub_exec, "{ns}.{name} must revoke PUBLIC EXECUTE");
        }
    }

    let guard_row = a
        .query_one(
            "SELECT p.oid, p.proowner,
                    encode(sha256(convert_to(replace(p.prosrc, E'\\r\\n', E'\\n'), 'UTF8')), 'hex'),
                    (SELECT r.proowner FROM pg_catalog.pg_proc r
                     JOIN pg_catalog.pg_namespace rn ON rn.oid = r.pronamespace
                     WHERE rn.nspname = 'execution' AND r.proname = 'run_start')
             FROM pg_catalog.pg_proc p
             JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
             WHERE n.nspname = 'execution' AND p.proname = 'assert_current_pack_terrain'",
            &[],
        )
        .unwrap();
    let guard_oid: u32 = guard_row.get(0);
    let guard_owner: u32 = guard_row.get(1);
    let guard_sha: String = guard_row.get(2);
    let run_start_owner: u32 = guard_row.get(3);
    assert_eq!(guard_owner, run_start_owner);
    for role in [
        "dw_runtime",
        "dw_m1_broker",
        "dw_m1_cognition",
        "dw_m1_terrain",
        "dw_m1_trajectory",
        "dw_m1_stop_writer",
    ] {
        let can_exec: bool = a
            .query_one(
                "SELECT has_function_privilege($1, $2::oid, 'EXECUTE')",
                &[&role, &guard_oid],
            )
            .unwrap()
            .get(0);
        assert!(
            !can_exec,
            "{role} must not execute assert_current_pack_terrain"
        );
    }
    let body_0038 = extract_fn_body(
        include_str!("../../migrations/0038_https_lab.sql"),
        "execution.assert_current_pack_terrain",
    );
    let body_0037 = extract_fn_body(
        include_str!("../../migrations/0037_https_loop.sql"),
        "execution.assert_current_pack_terrain",
    );
    assert_eq!(guard_sha, lab_support::sha256_hex(body_0038.as_bytes()));
    assert_ne!(guard_sha, lab_support::sha256_hex(body_0037.as_bytes()));

    let pk_sessions: String = a
        .query_one(
            "SELECT pg_catalog.pg_get_constraintdef(oid)
             FROM pg_catalog.pg_constraint
             WHERE conrelid = 'execution.live_effect_sessions'::regclass AND contype = 'p'",
            &[],
        )
        .unwrap()
        .get(0);
    assert_eq!(pk_sessions, "PRIMARY KEY (attempt)");

    let pk_control: String = a
        .query_one(
            "SELECT pg_catalog.pg_get_constraintdef(oid)
             FROM pg_catalog.pg_constraint
             WHERE conrelid = 'execution.live_effect_control'::regclass AND contype = 'p'",
            &[],
        )
        .unwrap()
        .get(0);
    assert_eq!(pk_control, "PRIMARY KEY (attempt)");

    let pk_events: String = a
        .query_one(
            "SELECT pg_catalog.pg_get_constraintdef(oid)
             FROM pg_catalog.pg_constraint
             WHERE conrelid = 'execution.live_effect_events'::regclass AND contype = 'p'",
            &[],
        )
        .unwrap()
        .get(0);
    assert_eq!(pk_events, "PRIMARY KEY (attempt, phase)");

    let step_unique: i64 = a
        .query_one(
            "SELECT count(*) FROM pg_catalog.pg_constraint
             WHERE conrelid = 'execution.live_dns_results'::regclass
               AND conname = 'live_dns_results_step_attempt_key'
               AND contype = 'u'",
            &[],
        )
        .unwrap()
        .get(0);
    assert_eq!(step_unique, 1);
}

#[ignore]
#[test]
fn schema0038_retains_legacy_live_dns_and_0037_fixture_paths() {
    let _guard = native::qualified_existing_db();
    let release = lab_support::sample_release_sha();
    let qref_legacy = Uuid::new_v4();
    seed_qual(
        qref_legacy,
        "autonomous-dns-genai-lab-v1",
        "LAB",
        HTTPS_LAB_RESOLVER,
        &release,
        now_ms() - 60_000,
        now_ms() + 3_600_000,
    );
    let e = Uuid::new_v4();
    let c = Uuid::new_v4();
    let run_legacy = Uuid::new_v4();
    let mut reg_legacy = base_registration_json(e, c, qref_legacy, &release);
    reg_legacy["m1_inference_permission"]["version"] = json!("m1_inference_permission_v6");
    reg_legacy["m1_inference_permission"]["run_profile"] = json!("autonomous-dns-genai-lab-v1");
    reg_legacy["m1_inference_permission"]["profile"] = json!("fixture-genai-responses-v1");
    reg_legacy["m1_inference_permission"]["operation_limits"]["output_tokens_per_call"] =
        json!(4096);
    reg_legacy["m1_inference_permission"]["operation_limits"]["decision_wall_time_ms"] =
        json!(129000);
    reg_legacy["m1_inference_permission"]["operation_limits"]["per_call_wall_time_ms"] =
        json!(62000);
    reg_legacy["m1_inference_permission"]["operation_limits"]["soft_completion_ms"] = json!(60000);
    reg_legacy["m1_inference_permission"]["operation_limits"]["shared_wall_time_ms"] =
        json!(124000);
    reg_legacy["m1_inference_permission"]["operation_limits"]["phase_wall_time_ms"] = json!(134000);
    reg_legacy["m1_permission"]["contact_rules"] = json!([{
        "asset_ref": reg_legacy["included_assets"][0],
        "rule": {"exact": "seed.duskweave.test"},
        "priority": 1
    }]);
    register_lab_campaign(&reg_legacy).unwrap();

    let req_legacy = json!({
        "version": "m1_autonomous_run_request_v4",
        "run_id": run_legacy,
        "run_profile": "autonomous-dns-genai-lab-v1",
        "execution_mode": "live",
        "inference_profile": "fixture-genai-responses-v1",
        "deadline_at_ms": now_ms() + 300_000,
        "live_binding": {
            "qualification_ref": qref_legacy,
            "release_sha256": release,
            "resolver_ipv4": HTTPS_LAB_RESOLVER
        },
        "context": {
            "version": "m1_context_prepare_request_v1",
            "mission": {
                "version": "m1_inference_reservation_request_v1",
                "engagement_id": e,
                "campaign_id": c,
                "operator_ref": reg_legacy["operator_ref"],
                "expected_mission_revision": 1,
                "permission_id": reg_legacy["m1_inference_permission"]["permission_id"],
                "goal_ref": reg_legacy["goal_ref"],
                "exercise_mode": "blind"
            },
            "candidates": [{"asset_ref": reg_legacy["included_assets"][0], "priority": "high"}]
        },
        "seed_names": [{"asset_ref": reg_legacy["included_assets"][0], "name": "seed.duskweave.test"}]
    });
    let parsed =
        duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&req_legacy).unwrap()).unwrap();
    let mut runs = PgRunStore::new(broker());
    if let Err(err) = runs.start(&parsed, &release) {
        let diag = broker()
            .query_one(
                "SELECT execution.run_start($1,$2)",
                &[&req_legacy, &release],
            )
            .err()
            .map(|e| crate::admission::db_err(&e))
            .unwrap_or_default();
        panic!("legacy run_start failed ({err:?}): {diag}");
    }
    let leg_summary = runs.summary(run_legacy).unwrap();
    assert_eq!(leg_summary["live_qualified"], json!(true));
    assert_eq!(leg_summary["execution_mode"], json!("live"));
    let _ = (PgEffectStore::new(broker()), LIVE_RUN_PROFILE);
    let _ = runs.stop(run_legacy);

    // Retained 0037 `controlled_fixture` `m1_https_loop_v1` continues working on 0038.
    let e_f = Uuid::new_v4();
    let c_f = Uuid::new_v4();
    let fix_run_id = Uuid::new_v4();
    let mut reg_fix = base_registration_json(e_f, c_f, Uuid::new_v4(), &release);
    let ip_fix = &mut reg_fix["m1_inference_permission"];
    ip_fix["version"] = json!("m1_https_loop_v1");
    ip_fix["run_profile"] = json!("m1_https_loop_v1");
    ip_fix["execution_mode"] = json!("controlled_fixture");
    ip_fix["profile"] = json!("fixture-genai-v4-pro-low-v2");
    ip_fix.as_object_mut().unwrap().remove("live_binding");
    register_lab_campaign(&reg_fix).unwrap();
    let req_fix_val = json!({
        "version": "m1_https_loop_v1",
        "run_id": fix_run_id,
        "run_profile": "m1_https_loop_v1",
        "execution_mode": "controlled_fixture",
        "inference_profile": "fixture-genai-v4-pro-low-v2",
        "deadline_at_ms": now_ms() + 600_000,
        "context": {
            "version": "m1_context_prepare_request_v1",
            "mission": {
                "version": "m1_inference_reservation_request_v1",
                "engagement_id": e_f,
                "campaign_id": c_f,
                "operator_ref": reg_fix["operator_ref"],
                "expected_mission_revision": 1,
                "permission_id": reg_fix["m1_inference_permission"]["permission_id"],
                "goal_ref": reg_fix["goal_ref"],
                "exercise_mode": "blind"
            },
            "candidates": [{"asset_ref": reg_fix["included_assets"][0], "priority": "high"}]
        },
        "seed_names": [{"asset_ref": reg_fix["included_assets"][0], "name": "head.duskweave.test"}]
    });
    let req_fix =
        duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&req_fix_val).unwrap()).unwrap();
    assert!(runs.start(&req_fix, &release).unwrap());
    let summary = runs.summary(fix_run_id).unwrap();
    assert_eq!(summary["execution_mode"], json!("controlled_fixture"));
    let _ = runs.stop(fix_run_id);

    let (_, _, lab_run, _, mut runs_lab, _) = start_lab_run(Uuid::new_v4());
    let step1 = runs_lab.https_step(lab_run, 1).unwrap();
    assert_eq!(step1.step_index, 1);
    let _ = runs_lab.stop(lab_run);
}

#[ignore]
#[test]
fn schema0038_privilege_fences_enforce_42501_across_roles() {
    let _guard = native::qualified_existing_db();
    seed_lab_qual(Uuid::new_v4());
    let roles = [
        ("runtime", "DW_TEST_DATABASE_URL"),
        ("broker", "DW_INFERENCE_BROKER_DATABASE_URL"),
        ("cognition", "DW_COGNITION_DATABASE_URL"),
        ("terrain", "DW_TERRAIN_DATABASE_URL"),
        ("trajectory", "DW_TRAJECTORY_DATABASE_URL"),
        ("stop_writer", "DW_STOP_WRITER_DATABASE_URL"),
    ];
    for (name, var) in roles {
        let mut c = native::client(var);
        for (tbl, allowed) in [
            (
                "execution.live_qualifications",
                name == "runtime" || name == "broker",
            ),
            ("execution.live_https_results", name == "broker"),
            ("execution.native_input_bindings", name == "broker"),
        ] {
            let sel = c.query_one(&format!("SELECT count(*) FROM {tbl}"), &[]);
            if allowed {
                assert!(sel.is_ok(), "{name} SELECT {tbl}");
            } else {
                assert_42501(sel, &format!("{name} SELECT {tbl}"));
            }
            assert_42501(
                c.execute(&format!("DELETE FROM {tbl} WHERE false"), &[]),
                &format!("{name} DELETE {tbl}"),
            );
            assert_42501(
                c.execute(&format!("INSERT INTO {tbl} DEFAULT VALUES"), &[]),
                &format!("{name} INSERT {tbl}"),
            );
        }
    }
}
