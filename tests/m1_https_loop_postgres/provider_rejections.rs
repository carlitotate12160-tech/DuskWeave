//! Cause-specific SQL admission witnesses for the actual Pro-v2 overlay.
//! Broker `run_start` is the admission actor; admin only seeds qualification.
use crate::provider_upgrade::{
    ACTUAL, BROKER, FIXTURE, FIXTURE_URL, actual_pro_v2_registered_with, ensure_0039_overlay,
    lab_permission, live_binding, permission_json, sql_matches,
};
use crate::terrain::terrain_support::native;
use duskweave::m1_inference_profile::{
    HTTPS_LAB_HOSTNAME, HTTPS_LAB_PURPOSE, HTTPS_LAB_RESOLVER, HTTPS_LAB_RUN_PROFILE,
};
use postgres::Client;
use serde_json::{Value, json};
use uuid::Uuid;

const BOGUS: &str = "bogus-profile";
const LEGAL_DEADLINE: &str = "SELECT execution.run_start(jsonb_set($1,'{deadline_at_ms}',to_jsonb(floor(extract(epoch FROM clock_timestamp())*1000)::bigint + 600000)),$2)";
const OVERLIMIT_DEADLINE: &str = "SELECT execution.run_start(jsonb_set($1,'{deadline_at_ms}',to_jsonb(floor(extract(epoch FROM clock_timestamp())*1000)::bigint + 1280000)),$2)";

fn release() -> String {
    "a".repeat(64)
}

fn run_id(request: &Value) -> Uuid {
    Uuid::parse_str(request["run_id"].as_str().unwrap()).unwrap()
}

fn assert_absent(broker: &mut Client, run: Uuid) {
    for table in [
        "execution.autonomous_runs",
        "execution.autonomous_control",
        "execution.https_loop_steps",
    ] {
        let count: i64 = broker
            .query_one(
                &format!("SELECT count(*)::bigint FROM {table} WHERE run_id=$1"),
                &[&run],
            )
            .unwrap()
            .get(0);
        assert_eq!(count, 0, "{table} committed a row for {run}");
    }
}

fn assert_refused(err: postgres::Error, message: &str) {
    let db = err
        .as_db_error()
        .unwrap_or_else(|| panic!("{message}: {err}"));
    assert_eq!(db.code().code(), "P0001", "{message}: {db}");
    assert_eq!(db.message(), message);
}

fn admit_rolled_back(broker: &mut Client, request: &Value) {
    let run = run_id(request);
    let mut tx = broker.transaction().unwrap();
    let started: bool = tx
        .query_one(LEGAL_DEADLINE, &[request, &release()])
        .unwrap()
        .get(0);
    assert!(started, "legal same-statement deadline must be admitted");
    let present: i64 = tx
        .query_one(
            "SELECT count(*)::bigint FROM execution.autonomous_runs WHERE run_id=$1",
            &[&run],
        )
        .unwrap()
        .get(0);
    assert_eq!(present, 1);
    tx.rollback().unwrap();
    assert_absent(broker, run);
}

fn refuse_run(broker: &mut Client, request: &Value, message: &str) {
    let err = broker
        .query_one("SELECT execution.run_start($1,$2)", &[request, &release()])
        .unwrap_err();
    assert_refused(err, message);
    assert_absent(broker, run_id(request));
}

fn seed_lab_qualification(qref: Uuid, release: &str) {
    let mut admin = native::admin();
    let now: i64 = admin
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    let material = "b".repeat(64);
    let resolver: std::net::IpAddr = HTTPS_LAB_RESOLVER.parse().unwrap();
    let from = now - 60_000;
    let until = now + 3_600_000;
    admin
        .execute(
            "INSERT INTO execution.live_qualifications \
             (qualification_ref,run_profile,purpose,release_sha256,launcher_sha256,\
              capture_sha256,exporter_sha256,image_digest,manifest_sha256,boot_id,\
              resolver_ipv4,valid_from_ms,valid_until_ms) \
             VALUES ($1,$2,$3,$4,$5,$5,$5,$6,$5,$7,$8,$9,$10)",
            &[
                &qref,
                &HTTPS_LAB_RUN_PROFILE,
                &HTTPS_LAB_PURPOSE,
                &release,
                &material,
                &format!("sha256:{material}"),
                &Uuid::from_u128(0xe511),
                &resolver,
                &from,
                &until,
            ],
        )
        .unwrap();
}

fn register_fixture_lab(slot: u128) -> Value {
    let qref = Uuid::new_v4();
    let release = release();
    seed_lab_qualification(qref, &release);
    let binding = json!({
        "qualification_ref": qref,
        "release_sha256": release,
        "resolver_ipv4": HTTPS_LAB_RESOLVER,
    });
    let mut request = crate::https_support::registered_with(slot, |raw| {
        raw["m1_permission"]["resolver_ipv4"] = json!(HTTPS_LAB_RESOLVER);
        raw["m1_permission"]["contact_rules"][0]["rule"] = json!({"exact": HTTPS_LAB_HOSTNAME});
        let permission = &mut raw["m1_inference_permission"];
        permission["profile"] = json!(FIXTURE);
        permission["recipient"] = json!(FIXTURE_URL);
        permission["run_profile"] = json!(HTTPS_LAB_RUN_PROFILE);
        permission["execution_mode"] = json!("live");
        permission["live_binding"] = binding.clone();
    });
    request["run_profile"] = json!(HTTPS_LAB_RUN_PROFILE);
    request["execution_mode"] = json!("live");
    request["inference_profile"] = json!(FIXTURE);
    request["live_binding"] = binding;
    assert_eq!(request["seed_names"][0]["name"], HTTPS_LAB_HOSTNAME);
    request
}

#[ignore]
#[test]
fn sql_permission_matrix_rejects_crossed_actual_and_lab_profiles() {
    let _guard = native::qualified_existing_db();
    ensure_0039_overlay();
    let mut broker = native::client(BROKER);
    for ok in [
        permission_json(ACTUAL, crate::provider_upgrade::ACTUAL_URL),
        permission_json(FIXTURE, FIXTURE_URL),
        lab_permission(permission_json(FIXTURE, FIXTURE_URL)),
    ] {
        assert!(sql_matches(&mut broker, &ok));
    }
    let mut with_live = permission_json(ACTUAL, crate::provider_upgrade::ACTUAL_URL);
    with_live["live_binding"] = live_binding();
    let mut with_purpose = permission_json(ACTUAL, crate::provider_upgrade::ACTUAL_URL);
    with_purpose["qualification_purpose"] = json!("HTTPS_LOOP_LAB");
    for bad in [
        permission_json(ACTUAL, FIXTURE_URL),
        permission_json(FIXTURE, crate::provider_upgrade::ACTUAL_URL),
        lab_permission(permission_json(ACTUAL, crate::provider_upgrade::ACTUAL_URL)),
        lab_permission(permission_json(ACTUAL, FIXTURE_URL)),
        permission_json(BOGUS, crate::provider_upgrade::ACTUAL_URL),
        with_live,
        with_purpose,
    ] {
        assert!(!sql_matches(&mut broker, &bad), "SQL must reject {bad}");
    }
}

#[ignore]
#[test]
fn run_start_refuses_crossed_profile_without_a_scoped_row() {
    let _guard = native::qualified_existing_db();
    ensure_0039_overlay();
    let mut broker = native::client(BROKER);
    let correct = actual_pro_v2_registered_with(0xe501, |_| {});
    admit_rolled_back(&mut broker, &correct);
    let mut crossed = correct;
    crossed["inference_profile"] = json!(FIXTURE);
    refuse_run(&mut broker, &crossed, "run_authority_unavailable");
}

#[ignore]
#[test]
fn run_start_accepts_fixture_lab_and_refuses_actual_profile() {
    let _guard = native::qualified_existing_db();
    ensure_0039_overlay();
    let mut broker = native::client(BROKER);
    let mut lab = register_fixture_lab(0xe502);
    admit_rolled_back(&mut broker, &lab);
    lab["inference_profile"] = json!(ACTUAL);
    refuse_run(&mut broker, &lab, "run_request_refused");
}

#[ignore]
#[test]
fn run_start_refuses_same_statement_overlimit_deadline() {
    let _guard = native::qualified_existing_db();
    ensure_0039_overlay();
    let mut broker = native::client(BROKER);
    let request = actual_pro_v2_registered_with(0xe503, |_| {});
    admit_rolled_back(&mut broker, &request);
    let err = broker
        .query_one(OVERLIMIT_DEADLINE, &[&request, &release()])
        .unwrap_err();
    assert_refused(err, "run_deadline_refused");
    assert_absent(&mut broker, run_id(&request));
}
