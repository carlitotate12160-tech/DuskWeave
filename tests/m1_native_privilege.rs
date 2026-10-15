#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_native_inference.rs"]
mod native;
use serde_json::{Value, json};
use uuid::Uuid;

#[test]
fn missing_dsns_fail_without_fallback_or_cluster_changes() {
    if std::env::var_os("DW_NATIVE_CONFIG_NEGATIVE").is_some() {
        native::setup();
        panic!("missing configuration must fail before setup");
    }
    let _guard = native::db();
    let before: i64 = native::admin()
        .query_one("SELECT count(*) FROM pg_roles", &[])
        .unwrap()
        .get(0);
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child
        .env_clear()
        .env("DW_NATIVE_CONFIG_NEGATIVE", "1")
        .args([
            "--exact",
            "missing_dsns_fail_without_fallback_or_cluster_changes",
            "--nocapture",
        ]);
    for variable in ["SystemRoot", "LLVM_PROFILE_FILE"] {
        if let Some(value) = std::env::var_os(variable) {
            child.env(variable, value);
        }
    }
    let result = child.output().unwrap();
    assert!(!result.status.success());
    let text = String::from_utf8_lossy(&result.stdout).into_owned()
        + &String::from_utf8_lossy(&result.stderr);
    assert!(text.contains("native_fixture_configuration_invalid"));
    assert_eq!(
        native::admin()
            .query_one("SELECT count(*) FROM pg_roles", &[])
            .unwrap()
            .get::<_, i64>(0),
        before
    );
}

#[test]
fn a_second_broker_cannot_rewrite_another_writers_accounting() {
    use duskweave::m1_reasoning::inference::Broker;
    let _guard = native::db();
    let mut case = native::decision(0x9803);
    let r = case.broker.reserve(&case.scope, &case.pack).unwrap();
    let b = duskweave::m1_reasoning::request::bind(&case.pack, None).unwrap();
    case.broker
        .may_send(&case.scope, r.generation_call, &b)
        .unwrap();
    let mut other = native::client("DW_NATIVE_OTHER_BROKER_DATABASE_URL");
    for state in [
        "completed",
        "cancelled_uncertain",
        "usage_unknown",
        "failed_never_released",
    ] {
        assert!(
            other
                .execute(
                    "SELECT execution.inference_call_state($1,$2,NULL)",
                    &[&r.generation_call, &state]
                )
                .is_err(),
            "foreign accounting state: {state}"
        );
    }
    assert_eq!(
        native::admin()
            .query_one(
                "SELECT state FROM execution.inference_calls WHERE call=$1",
                &[&r.generation_call]
            )
            .unwrap()
            .get::<_, String>(0),
        "may_send_uncertain"
    );
}

#[test]
fn accounting_refuses_a_call_without_its_reservation_writer() {
    let _guard = native::db();
    let mut a = native::admin();
    let call: Uuid = a.query_one("SELECT gen_random_uuid()", &[]).unwrap().get(0);
    a.execute("INSERT INTO execution.inference_calls VALUES(gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),$1,'fixture','{}','{}','{}','may_send_uncertain',NULL)",&[&call]).unwrap();
    let mut other = native::client("DW_NATIVE_OTHER_BROKER_DATABASE_URL");
    let result = other.execute(
        "SELECT execution.inference_call_state($1,'completed',NULL)",
        &[&call],
    );
    assert!(
        result.is_err(),
        "accounting requires the exact reservation writer, not just Broker membership"
    );
}

#[test]
fn native_input_binding_is_immutable_and_writer_scoped() {
    let _guard = native::db();
    let (_, q) = native::registered(0x9802);
    let e: Uuid = serde_json::from_value(q["mission"]["engagement_id"].clone()).unwrap();
    let c: Uuid = serde_json::from_value(q["mission"]["campaign_id"].clone()).unwrap();
    let mut broker = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let d: Uuid = broker
        .query_one("SELECT gen_random_uuid()", &[])
        .unwrap()
        .get(0);
    let hash = "a".repeat(64);
    for _ in 0..2 {
        broker
            .execute(
                "SELECT execution.bind_inference_input($1,$2,$3,$4)",
                &[&e, &c, &d, &hash],
            )
            .expect("native input must bind before reservation");
    }
    assert!(
        broker
            .execute(
                "SELECT execution.bind_inference_input($1,$2,$3,$4)",
                &[&e, &c, &d, &"b".repeat(64)]
            )
            .is_err()
    );
    let mut other = native::client("DW_NATIVE_OTHER_BROKER_DATABASE_URL");
    assert!(
        other
            .execute(
                "SELECT execution.bind_inference_input($1,$2,$3,$4)",
                &[&e, &c, &d, &hash]
            )
            .is_err()
    );
}

#[test]
fn v2_consent_refuses_missing_repair_and_unsupported_fixed_limits() {
    let _guard = native::db();
    let (valid, _) = native::registered(0x9801);
    let mut missing = valid.clone();
    missing["m1_inference_permission"]["field_classes"]
        .as_array_mut()
        .unwrap()
        .retain(|c| c != "admitted_repair_fields_codes");
    assert!(
        duskweave::input::parse_register(&serde_json::to_vec(&missing).unwrap()).is_err(),
        "v2 repair consent is mandatory"
    );
    for (field, value) in [
        ("json_depth", 8),
        ("collection_entries", 32),
        ("response_bytes", 32768),
        ("proposal_bytes", 4096),
        ("text_field_bytes", 512),
        ("repair_bytes", 1024),
        ("repair_error_codes", 4),
        ("connect_time_ms", 1000),
    ] {
        let mut bad = valid.clone();
        bad["m1_inference_permission"]["operation_limits"][field] = json!(value);
        assert!(
            duskweave::input::parse_register(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "unsupported fixed v2 limit: {field}"
        );
    }
}

#[test]
fn raw_sql_reservation_and_repair_recheck_v2_consent() {
    use duskweave::m1_reasoning::{inference::Broker, request};
    let _guard = native::db();
    for (index, path, value) in [
        (
            0,
            "field_classes",
            json!([
                "mission_question_constraints",
                "scoped_opaque_references",
                "omissions_unknown",
                "capacity",
                "admitted_operator_premises"
            ]),
        ),
        (1, "operation_limits,json_depth", json!(8)),
        (2, "operation_limits,connect_time_ms", json!(1000)),
    ] {
        for repair in [false, true] {
            let mut case = native::decision(0x9820 + index * 2 + u128::from(repair));
            let call = if repair {
                let r = case.broker.reserve(&case.scope, &case.pack).unwrap();
                let generation = request::bind(&case.pack, None).unwrap();
                case.broker
                    .may_send(&case.scope, r.generation_call, &generation)
                    .unwrap();
                case.broker
                    .client
                    .execute(
                        "SELECT execution.inference_call_state($1,'completed',NULL)",
                        &[&r.generation_call],
                    )
                    .unwrap();
                Some(r.repair_call)
            } else {
                None
            };
            let mut admin = native::admin();
            let field = format!("{{fields,m1_inference_permission,{path}}}");
            admin.execute("UPDATE mission.registration_outbox SET contract=jsonb_set(contract,$3::text::text[],$4) WHERE engagement_id=$1 AND campaign_id=$2",&[&case.scope.engagement,&case.scope.campaign,&field,&value]).unwrap();
            admin.execute("UPDATE trajectory.registration_history h SET contract=o.contract FROM mission.registration_outbox o WHERE h.engagement_id=o.engagement_id AND h.campaign_id=o.campaign_id AND h.engagement_id=$1 AND h.campaign_id=$2 AND h.status='accepted'",&[&case.scope.engagement,&case.scope.campaign]).unwrap();
            let result = if let Some(call) = call {
                let b = request::bind(&case.pack, Some(&["json_structure"])).unwrap();
                case.broker.client.execute(
                    "SELECT execution.inference_may_send($1,$2,$3,$4,$5,$6,$7,$8)",
                    &[
                        &case.scope.engagement,
                        &case.scope.campaign,
                        &case.scope.decision,
                        &call,
                        &std::str::from_utf8(&b.pack).unwrap(),
                        &std::str::from_utf8(&b.configuration).unwrap(),
                        &std::str::from_utf8(&b.body).unwrap(),
                        &b.digest,
                    ],
                )
            } else {
                assert_eq!(admin.query_one("SELECT count(*) FROM execution.inference_reservations WHERE engagement_id=$1 AND campaign_id=$2",&[&case.scope.engagement,&case.scope.campaign]).unwrap().get::<_,i64>(0),0,"no active hold may mask consent refusal");
                let source: Value = case
                    .broker
                    .client
                    .query_one(
                        "SELECT mission.current_inference_publication($1,$2,$3)",
                        &[
                            &case.scope.engagement,
                            &case.scope.campaign,
                            &case.scope.decision,
                        ],
                    )
                    .unwrap()
                    .get(0);
                assert!(
                    !source.is_null(),
                    "tampered consent must retain a qualified source"
                );
                case.broker.client.execute(
                    "SELECT execution.inference_reservation('reserve',$1,$2,$3)",
                    &[&case.scope.decision, &case.request["mission"], &source],
                )
            };
            let expected = if !repair && index == 0 {
                "inference_input_or_consent_refused"
            } else {
                "inference_authority_unavailable"
            };
            assert_eq!(
                result.unwrap_err().as_db_error().unwrap().message(),
                expected,
                "raw SQL consent boundary must be the reason for refusal"
            );
            assert_eq!(
                admin
                    .query_one(
                        "SELECT count(*) FROM execution.inference_calls WHERE d=$1",
                        &[&case.scope.decision]
                    )
                    .unwrap()
                    .get::<_, i64>(0),
                i64::from(repair)
            );
        }
    }
}

#[test]
fn arbitrary_anomaly_without_committed_source_is_refused() {
    let _guard = native::db();
    let mut h = native::client("DW_TRAJECTORY_DATABASE_URL");
    let mut tx = h.transaction().unwrap();
    let result = tx.execute("INSERT INTO trajectory.reasoning_history(engagement_id,campaign_id,operation_id,event_id,status,anomaly_category) VALUES(gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),'anomaly','conflicting_identity')",&[]);
    tx.rollback().unwrap();
    assert!(
        result.is_err(),
        "unverified anomaly must not poison history"
    );
}

fn decision() -> Value {
    json!({"kind":"reasoning_decision","occurred_at_ms":1,"recorded_at_ms":2,
        "source_mode":"blind","provenance":null,"event":Uuid::from_u128(1),
        "engagement":Uuid::from_u128(2),"campaign":Uuid::from_u128(3),"decision":Uuid::from_u128(4),
        "producer":"cognition","version":1,"mission_revision":1,"permission":Uuid::from_u128(5),
        "call_ids":[],"bindings":[],"proposal":null,"disposition":"reference_ineligible",
        "publication_obligation":"trajectory.reasoning_history.v1"})
}

#[test]
fn direct_cognition_publication_refuses_unknown_metadata_atomically() {
    let _guard = native::db();
    native::setup();
    let mut c = native::client("DW_COGNITION_DATABASE_URL");
    let event: Uuid = c.query_one("SELECT gen_random_uuid()", &[]).unwrap().get(0);
    let mut x = decision();
    x["event"] = json!(event);
    x["extra_metadata"] = json!(true);
    let result = c.execute(
        "SELECT cognition.publish_reasoning($1,$2)",
        &[&x, &"fixture-input"],
    );
    assert_eq!(
        result.unwrap_err().as_db_error().unwrap().message(),
        "inference_source_refused",
        "unknown metadata must be rejected before identity handling"
    );
    let mut a = native::admin();
    assert_eq!(
        a.query_one(
            "SELECT count(*) FROM cognition.reasoning_decisions WHERE event=$1",
            &[&event]
        )
        .unwrap()
        .get::<_, i64>(0),
        0
    );
    assert_eq!(
        a.query_one(
            "SELECT count(*) FROM cognition.reasoning_outbox WHERE event=$1",
            &[&event]
        )
        .unwrap()
        .get::<_, i64>(0),
        0
    );
}

#[test]
fn direct_history_insert_cannot_forge_accepted_source() {
    let _guard = native::db();
    native::setup();
    let mut h = native::client("DW_TRAJECTORY_DATABASE_URL");
    let mut tx = h.transaction().unwrap();
    let result = tx.execute("INSERT INTO trajectory.reasoning_history(engagement_id,campaign_id,operation_id,event_id,status,contract,completed_at) VALUES($1,$2,$3,$4,'accepted',$5,clock_timestamp())",&[&Uuid::from_u128(102),&Uuid::from_u128(103),&Uuid::from_u128(104),&Uuid::from_u128(101),&decision()]);
    tx.rollback().unwrap();
    assert!(
        result.is_err(),
        "direct SQL must not create completed history without committed source"
    );
}

#[test]
fn history_owner_is_narrow_and_cannot_publish_or_mutate_mission() {
    let _guard = native::db();
    native::setup();
    let mut h = native::client("DW_TRAJECTORY_DATABASE_URL");
    let row = h.query_one("SELECT pg_has_role(session_user,'dw_m1_trajectory','MEMBER'),pg_has_role(session_user,'dw_runtime','MEMBER'),has_table_privilege(current_user,'trajectory.reasoning_history','INSERT'),has_table_privilege(current_user,(SELECT t.oid FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace WHERE n.nspname='mission' AND t.relname='missions'),'INSERT')",&[]).unwrap();
    assert!(row.get::<_, bool>(0));
    assert!(!row.get::<_, bool>(1));
    assert!(row.get::<_, bool>(2));
    assert!(!row.get::<_, bool>(3));
    assert!(
        h.execute(
            "SELECT cognition.publish_reasoning($1,$2)",
            &[&decision(), &"fixture-input"]
        )
        .is_err()
    );
    assert!(h.batch_execute("SET ROLE dw_runtime").is_err());
    assert!(
        h.batch_execute("UPDATE mission.missions SET owner_revision=owner_revision")
            .is_err()
    );
    assert!(
        h.batch_execute("DELETE FROM trajectory.reasoning_history")
            .is_err()
    );
    assert!(
        h.batch_execute("TRUNCATE cognition.reasoning_outbox")
            .is_err()
    );
}
