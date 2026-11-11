//! Schema-0030 successor pack compatibility matrix: real registration,
//! typed run request, Context producer and restricted marker consumer for
//! retained v2/v3 and v4 LAB/LIVE runs. SQL-fixture qualifications are
//! synthetic bindings to their accepted inputs, never host evidence.
use super::fixture::*;
use super::run_support::*;
use duskweave::m1_reasoning::{
    inference::{Broker, Reservation, Scope},
    inference_intent,
    request::{self, BoundCall},
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// The exact key set assert_successor_pack enforces for every successor
/// request version; live_binding/qualification_purpose are v4-only.
#[rustfmt::skip]
const COMMON_KEYS: [&str; 22] = [
    "version", "engagement_id", "campaign_id", "decision_id", "permission_id",
    "mission_revision", "goal_ref", "exercise_mode", "scope", "question",
    "evaluated_at", "owner_views", "references", "candidates", "omissions",
    "pending_unknown", "manifests", "remaining_capacity", "execution_mode",
    "run_profile", "inference_profile", "predecessor",
];

/// Real bound call for the reservation's generation call; the SDK intent
/// path carries an empty body, the legacy path its full wire body.
fn bind_generation(scope: &Scope, pack: &str, reservation: &Reservation, sdk: bool) -> BoundCall {
    if sdk {
        inference_intent::bind(
            scope,
            reservation,
            reservation.generation_call,
            pack.as_bytes(),
            None,
        )
        .expect("SDK generation binding")
    } else {
        request::bind(pack.as_bytes(), None).expect("legacy generation binding")
    }
}

fn pack_keys(pack: &Value, live: bool) {
    for key in COMMON_KEYS {
        assert!(pack.get(key).is_some(), "pack missing common key {key}");
    }
    let expected: std::collections::BTreeSet<&str> = COMMON_KEYS
        .into_iter()
        .chain(if live {
            ["live_binding", "qualification_purpose"].into_iter()
        } else {
            Default::default()
        })
        .collect();
    let actual: std::collections::BTreeSet<&str> = pack
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(actual, expected, "pack key set must close exactly");
}

fn without(pack: &Value, key: &str) -> Value {
    let mut q = pack.clone();
    q.as_object_mut().unwrap().remove(key);
    q
}
fn with(pack: &Value, key: &str, value: Value) -> Value {
    let mut q = pack.clone();
    q[key] = value;
    q
}
fn binding_with(pack: &Value, key: &str, value: Value) -> Value {
    let mut q = pack.clone();
    q["live_binding"][key] = value;
    q
}
fn synthetic_binding() -> Value {
    json!({"qualification_ref":Uuid::new_v4(),"release_sha256":RELEASE,
        "resolver_ipv4":"192.0.2.53"})
}

/// Direct restricted-SQL marker attempt: the intended refusal is exactly
/// P0001/run_pack_source_refused, never a sibling or transport category.
fn refused_marker(scope: &Scope, call: Uuid, pack: &Value, bound: &BoundCall, why: &str) {
    let mut client = broker();
    let error = match client.query_one(
        "SELECT execution.inference_may_send($1,$2,$3,$4,$5,$6,$7,$8)",
        &[
            &scope.engagement,
            &scope.campaign,
            &scope.decision,
            &call,
            &serde_json::to_string(pack).unwrap(),
            &String::from_utf8_lossy(&bound.configuration).into_owned(),
            &String::from_utf8_lossy(&bound.body).into_owned(),
            &bound.digest,
        ],
    ) {
        Ok(_) => panic!("{why}: mutated pack must be refused"),
        Err(error) => error,
    };
    assert_eq!(
        error.code().map(postgres::error::SqlState::code),
        Some("P0001"),
        "{why}"
    );
    assert_eq!(db_message(&error), "run_pack_source_refused", "{why}");
}

fn nonlive_negatives(pack: &Value) -> Vec<(Value, &'static str)> {
    vec![
        (
            with(pack, "live_binding", synthetic_binding()),
            "carrying live_binding",
        ),
        (
            with(pack, "qualification_purpose", json!("LAB")),
            "carrying qualification_purpose",
        ),
        (
            with(
                &with(pack, "live_binding", synthetic_binding()),
                "qualification_purpose",
                json!("LAB"),
            ),
            "carrying both live-only fields",
        ),
        (without(pack, "question"), "missing common field question"),
        (
            with(pack, "injected_field", json!(true)),
            "carrying unknown field",
        ),
    ]
}

fn live_negatives(pack: &Value) -> Vec<(Value, &'static str)> {
    let wrong_purpose = if pack["qualification_purpose"] == "LAB" {
        "LIVE"
    } else {
        "LAB"
    };
    let wrong_resolver = if pack["live_binding"]["resolver_ipv4"] == "192.0.2.53" {
        "9.9.9.9"
    } else {
        "192.0.2.53"
    };
    vec![
        (without(pack, "live_binding"), "missing live_binding"),
        (
            without(pack, "qualification_purpose"),
            "missing qualification_purpose",
        ),
        (
            without(&without(pack, "live_binding"), "qualification_purpose"),
            "missing both live-only fields",
        ),
        (
            binding_with(pack, "qualification_ref", json!(Uuid::new_v4())),
            "mismatched binding qualification_ref",
        ),
        (
            binding_with(pack, "resolver_ipv4", json!(wrong_resolver)),
            "mismatched binding resolver",
        ),
        (
            with(pack, "qualification_purpose", json!(wrong_purpose)),
            "mismatched qualification_purpose",
        ),
        (without(pack, "question"), "missing common field question"),
        (
            with(pack, "injected_field", json!(true)),
            "carrying unknown field",
        ),
    ]
}

fn marker(scope: &Scope, call: Uuid) -> postgres::Row {
    admin()
        .query_one(
            "SELECT binding,state FROM execution.inference_calls WHERE e=$1 AND c=$2 AND d=$3 AND call=$4",
            &[&scope.engagement, &scope.campaign, &scope.decision, &call],
        )
        .unwrap()
}
fn call_count(scope: &Scope) -> i64 {
    admin()
        .query_one(
            "SELECT count(*) FROM execution.inference_calls WHERE e=$1 AND c=$2 AND d=$3",
            &[&scope.engagement, &scope.campaign, &scope.decision],
        )
        .unwrap()
        .get(0)
}
fn effect_count(run: Uuid) -> i64 {
    admin()
        .query_one(
            "SELECT (SELECT count(*) FROM execution.fixture_dns_attempts WHERE run_id=$1)+(SELECT count(*) FROM execution.fixture_dns_results WHERE run_id=$1)+(SELECT count(*) FROM execution.live_effect_sessions WHERE run_id=$1)+(SELECT count(*) FROM execution.live_dns_results WHERE run_id=$1)",
            &[&run],
        )
        .unwrap()
        .get(0)
}

/// One matrix case: register, start the typed run, produce the real pack,
/// reserve, bind, admit through the restricted consumer, then refusals.
fn matrix_case(
    slot: u128,
    shape: impl FnOnce(&mut Value),
    version: &str,
    pack_version: &str,
    sdk: bool,
    live: bool,
) {
    let (e, c, _operator) = register_shaped(slot, shape);
    let run = Uuid::new_v4();
    let publication = live_publication(e, c, run);
    if version == "m1_autonomous_run_request_v4" {
        seed_live_qualification(&publication);
    }
    let mut runs = duskweave::postgres_m1_run::PgRunStore::new(broker());
    assert!(
        runs.start(&successor_request(&publication, run, version), RELEASE)
            .unwrap()
    );
    let record = runs.inspect(run).unwrap();
    let custodian = Renewals::start(run);
    let prepared = prepare_decision_one(&mut runs, &record);
    let scope = prepared.scope.clone();
    let pack_text = prepared.pack.utf8().to_owned();
    let pack: Value = serde_json::from_str(&pack_text).unwrap();
    assert_eq!(pack["version"], pack_version);
    pack_keys(&pack, live);
    assert_eq!(
        pack["inference_profile"].as_str().unwrap(),
        record.request.inference_profile.as_deref().unwrap()
    );

    let deadline = Instant::now() + Duration::from_millis(prepared.window_ms as u64);
    let mut port = broker_port(&record, deadline);
    let reservation = port.reserve(&scope, pack_text.as_bytes()).expect("reserve");
    let bound = bind_generation(&scope, &pack_text, &reservation, sdk);
    assert_eq!(bound.body.is_empty(), sdk, "SDK intents carry no wire body");
    port.may_send(&scope, reservation.generation_call, &bound)
        .unwrap_or_else(|error| {
            panic!("{version} pack must pass the restricted consumer: {error}")
        });
    let row = marker(&scope, reservation.generation_call);
    assert_eq!(row.get::<_, String>(0), bound.digest);
    assert_eq!(row.get::<_, String>(1), "may_send_uncertain");

    for (mutated, why) in if live {
        live_negatives(&pack)
    } else {
        nonlive_negatives(&pack)
    } {
        refused_marker(&scope, reservation.generation_call, &mutated, &bound, why);
    }
    assert_eq!(call_count(&scope), 1, "refused attempts add no call rows");
    assert_eq!(effect_count(run), 0, "refused attempts add no effect rows");

    // Honest no-send retirement: the durable generation marker settles to
    // failed_never_released with zero provider contact.
    port.never_released(&scope, reservation.generation_call, "no_send_retired")
        .expect("marker retirement");
    let row = marker(&scope, reservation.generation_call);
    assert_eq!(row.get::<_, String>(1), "failed_never_released");
    custodian.retire();
    runs.stop(run).unwrap();
}

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_successor_pack_v2_v3_v4_contract_matrix() {
    assert!(worker());
    let identity = successor_pack_identity();

    // Retained v2: permission v4, Legacy profile and fixture recipient.
    matrix_case(
        0x6c02,
        |raw| {
            let p = &mut raw["m1_inference_permission"];
            p["version"] = json!("m1_inference_permission_v4");
            p["profile"] = json!("fixture-responses-v1");
            p["recipient"] = json!("urn:duskweave:fixture:responses:v1");
        },
        "m1_autonomous_run_request_v2",
        "m1_context_pack_autonomous_v2",
        false,
        false,
    );

    // Retained v3: permission v5, Genai fixture profile, SDK intent binding.
    matrix_case(
        0x6c03,
        |_| {},
        "m1_autonomous_run_request_v3",
        "m1_context_pack_autonomous_v3",
        true,
        false,
    );

    // v4 LAB: permission v6, LAB profile, accepted binding and qualification.
    matrix_case(
        0x6c04,
        |raw| {
            let p = &mut raw["m1_inference_permission"];
            p["version"] = json!("m1_inference_permission_v6");
            p["execution_mode"] = json!("live");
            p["run_profile"] = json!(LAB);
            p["live_binding"] = json!({"qualification_ref":Uuid::new_v4(),
                "release_sha256":RELEASE,"resolver_ipv4":"192.0.2.53"});
        },
        "m1_autonomous_run_request_v4",
        "m1_context_pack_autonomous_v4",
        true,
        true,
    );

    // v4 LIVE: permission v6, LIVE profile/resolver and its own binding.
    matrix_case(
        0x6c05,
        |raw| {
            let p = &mut raw["m1_inference_permission"];
            p["version"] = json!("m1_inference_permission_v6");
            p["execution_mode"] = json!("live");
            p["run_profile"] = json!("autonomous-dns-genai-live-v1");
            p["profile"] = json!("deepseek-genai-responses-low-v1");
            p["recipient"] = json!("https://api.deepseek.com/responses");
            p["live_binding"] = json!({"qualification_ref":Uuid::new_v4(),
                "release_sha256":RELEASE,"resolver_ipv4":"9.9.9.9"});
            raw["m1_permission"]["resolver_ipv4"] = json!("9.9.9.9");
        },
        "m1_autonomous_run_request_v4",
        "m1_context_pack_autonomous_v4",
        true,
        true,
    );

    assert_eq!(
        successor_pack_identity(),
        identity,
        "installed0030 assert_successor_pack must be unchanged by the case"
    );
}
