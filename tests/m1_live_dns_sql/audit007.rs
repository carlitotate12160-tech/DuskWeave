//! DW-AUD-007 restricted-caller catalog-resolution cases: original-writer
//! isolation at execution.run_current/run_record, inference_call_state and
//! live_effect_stop, trusted role qualification at the public Rust ports and
//! effective search_path safety for the exact installed functions. The
//! temporary catalog objects are session-local test artifacts inside a
//! TEMP-eligible restricted fixture login; no production bypass, fixture
//! provisioner or runtime API is used.
use super::audit007_support::{
    LeaseGuard, altered_function_metadata, drop_catalog_shadows, empty_catalog_shadows,
    other_broker, scoped_snapshot, session_metadata, shadow_writer_oid, terrain_column, try_call,
};
use super::fixture::*;
use super::run_support::*;
use serde_json::{Value, json};
use uuid::Uuid;

/// Attempts through restricted Broker sessions: the control on the shared
/// session, then a fresh session whose pg_temp.pg_roles shadow (mapping
/// session_user to the victim writer oid, readable by function owners)
/// exists before its first catalog-touching call so no pre-shadow plan is
/// reused, then a warmed-session control after the shadow is dropped.
fn shadowed_boundary(
    foreign: &mut postgres::Client,
    writer: postgres::types::Oid,
    call: impl Fn(&mut postgres::Client) -> Value,
) -> Value {
    let control = call(foreign);
    let mut shadowed_session = other_broker();
    shadow_writer_oid(&mut shadowed_session, writer);
    let shadowed = call(&mut shadowed_session);
    drop_catalog_shadows(&mut shadowed_session);
    let warmed = call(&mut shadowed_session);
    json!({"control": control, "shadowed": shadowed, "warmed": warmed})
}

#[test]
#[ignore = "runs only inside the dedicated schema fixture worker"]
fn sql_live_aud007_original_writer_isolation_survives_temp_pg_roles() {
    assert!(worker());
    use duskweave::postgres_m1_run::PgRunStore;

    let (e, c, _op) = register_live(0xa071, true);
    let run = Uuid::new_v4();
    let publication = live_publication(e, c, run);
    seed_live_qualification(&publication);
    let mut runs = PgRunStore::new(broker());
    assert!(
        runs.start(
            &successor_request(&publication, run, "m1_autonomous_run_request_v4"),
            RELEASE
        )
        .unwrap()
    );
    let _lease = LeaseGuard::start(run);
    runs.renew(run).unwrap();

    // Broker A owns a current run; both supported owner reads succeed.
    let mut owner = broker();
    let writer: postgres::types::Oid = owner
        .query_one(
            "SELECT oid FROM pg_catalog.pg_roles WHERE rolname=session_user",
            &[],
        )
        .unwrap()
        .get(0);
    let current = owner
        .query_one("SELECT execution.run_current($1)", &[&run])
        .unwrap()
        .get::<_, Value>(0);
    assert!(current["fields"]["m1_inference_permission"].is_object());
    let record = owner
        .query_one("SELECT execution.run_record($1)", &[&run])
        .unwrap()
        .get::<_, Value>(0);
    assert_eq!(record["status"].as_str(), Some("running"));

    // Fresh restricted Broker session: a real login, dw_m1_broker member,
    // no power flags and TEMP-eligible on the fixture database.
    let mut foreign = other_broker();
    let meta = session_metadata(&mut foreign);
    eprintln!("aud007_session {}", meta);
    assert_eq!(meta["rolsuper"], false);
    assert_eq!(meta["rolbypassrls"], false);
    assert_eq!(meta["rolcreatedb"], false);
    assert_eq!(meta["rolcreaterole"], false);
    assert!(
        meta["temp_eligible"].as_bool().unwrap(),
        "restricted Broker login must hold TEMP on the fixture database; report the provisioning gap otherwise"
    );
    assert!(
        meta["member_of"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r == "dw_m1_broker")
    );

    runs.renew(run).unwrap();
    let observed = json!({
        "run_current": shadowed_boundary(&mut foreign, writer, |f| {
            try_call(f, "SELECT execution.run_current($1)", &[&run])
        }),
        "run_record": shadowed_boundary(&mut foreign, writer, |f| {
            try_call(f, "SELECT execution.run_record($1)", &[&run])
        }),
    });
    eprintln!("aud007_observed {observed}");

    // Original-writer refusal, not an expired lease or missing permission:
    // the same session repeats before, under and after the shadow.
    let record_refused = |v: &Value| v["ok"] == true && v["value_present"] == false;
    let current_refused = |v: &Value| v["ok"] == false && v["error"] == "run_not_current";
    assert!(record_refused(&observed["run_record"]["control"]));
    assert!(current_refused(&observed["run_current"]["control"]));
    assert!(record_refused(&observed["run_record"]["warmed"]));
    assert!(current_refused(&observed["run_current"]["warmed"]));
    assert!(
        record_refused(&observed["run_record"]["shadowed"]),
        "temp-shadowed foreign read must not return Broker A's run record: {observed}"
    );
    assert!(
        current_refused(&observed["run_current"]["shadowed"]),
        "temp-shadowed foreign read must not return Broker A's current run: {observed}"
    );

    runs.renew(run).unwrap();
    runs.stop(run).unwrap();
}

#[test]
#[ignore = "runs only inside the dedicated schema fixture worker"]
fn sql_live_aud007_inference_and_live_stop_refuse_foreign_writer() {
    assert!(worker());
    use super::receipt::{LabBasis, LiveProvider};
    use duskweave::m1_effect_session::EffectStore;
    use duskweave::m1_reasoning::{
        inference,
        postgres::{Basis, PgDecisionStore},
        postgres_terrain::TerrainBasis,
        request,
    };
    use duskweave::m1_run::RunPhase;
    use duskweave::postgres_m1_effect::PgEffectStore;
    use duskweave::postgres_m1_run::PgRunStore;
    use std::time::{Duration, Instant};

    // Actual admitted LAB session through supported SQL admission: real D1
    // through the restricted broker, then reserve through the real effect
    // store. The deterministic LiveProvider/LabBasis pair is reused exactly
    // as in the retained lifecycle case.
    let (e, c, _op) = register_live(0xa072, true);
    let run = Uuid::new_v4();
    let publication = live_publication(e, c, run);
    seed_live_qualification(&publication);
    let mut runs = PgRunStore::new(broker());
    assert!(
        runs.start(
            &successor_request(&publication, run, "m1_autonomous_run_request_v4"),
            RELEASE
        )
        .unwrap()
    );
    let record = runs.inspect(run).unwrap();
    let _lease = LeaseGuard::start(run);
    runs.renew(run).unwrap();
    let prepared = prepare_decision_one(&mut runs, &record);
    let deadline = Instant::now() + Duration::from_millis(prepared.window_ms as u64);
    let mut port = broker_port(&record, deadline);
    let mut provider = LiveProvider::default();
    let mut basis = LabBasis(TerrainBasis {
        mission: Basis {
            client: broker(),
            scope: prepared.scope.clone(),
            references: request::reference_ids(prepared.pack.utf8().as_bytes()).unwrap(),
            deadline,
        },
        dependencies: prepared.pack.terrain_dependencies().to_vec(),
    });
    let receipt = inference::infer(
        &mut port,
        &mut provider,
        &mut basis,
        &mut PgDecisionStore {
            client: client("DW_COGNITION_DATABASE_URL"),
            input_hash: prepared.input_hash.clone(),
        },
        &mut duskweave::postgres_trajectory::PgTrajectory::new(client(
            "DW_TRAJECTORY_DATABASE_URL",
        )),
        inference::InferenceInput {
            scope: &prepared.scope,
            pack: prepared.pack.utf8().as_bytes(),
            event_id: Uuid::new_v4(),
            occurred_at_ms: now_ms() as u64,
            source_mode: "blind".into(),
        },
    );
    assert_eq!(receipt.proposal_disposition, "validated_proposal");
    let completed_call = receipt.call_ids[0];
    runs.renew(run).unwrap();
    runs.complete_phase(run, RunPhase::DecisionOne).unwrap();
    runs.start_phase(run, RunPhase::Acquisition).unwrap();
    runs.claim_phase(run, RunPhase::Acquisition, RELEASE)
        .unwrap();
    let mut store = PgEffectStore::new(broker());
    runs.renew(run).unwrap();
    let session = store.reserve(run).unwrap();
    let generation = session.request.session_generation as i64;

    let writer: postgres::types::Oid = broker()
        .query_one(
            "SELECT oid FROM pg_catalog.pg_roles WHERE rolname=session_user",
            &[],
        )
        .unwrap()
        .get(0);
    let before = scoped_snapshot(run, e, c);
    let mut foreign = other_broker();
    let meta = session_metadata(&mut foreign);
    eprintln!("aud007_session {meta}");
    assert!(meta["temp_eligible"].as_bool().unwrap());

    let no_usage: Option<Value> = None;
    let observed = json!({
        "call_state_retire": shadowed_boundary(&mut foreign, writer, |f| {
            try_call(f, "SELECT execution.inference_call_state($1,$2,$3)", &[
                &completed_call, &"failed_never_released", &no_usage,
            ])
        }),
        "call_state_usage": shadowed_boundary(&mut foreign, writer, |f| {
            try_call(f, "SELECT execution.inference_call_state($1,$2,$3)", &[
                &completed_call, &"usage_unknown", &no_usage,
            ])
        }),
        "live_effect_stop": shadowed_boundary(&mut foreign, writer, |f| {
            try_call(f, "SELECT execution.live_effect_stop($1,$2,$3)", &[&run, &generation, &false])
        }),
    });
    let after = scoped_snapshot(run, e, c);
    eprintln!("aud007_observed {observed}");

    // Owner A still performs every lifecycle transition after refused
    // foreign attempts; the completed call above is the settled-call owner
    // success through the same inference_call_state path.
    store.stop(&session).unwrap();
    store.close_disposed(&session).unwrap();

    let state_refused = |v: &Value| v["ok"] == false && v["error"] == "inference_state_refused";
    let stop_refused = |v: &Value| v["ok"] == false && v["error"] == "live_stop_refused";
    for key in ["control", "warmed", "shadowed"] {
        assert!(
            state_refused(&observed["call_state_retire"][key]),
            "foreign settlement attempt must be refused at {key}: {observed}"
        );
        assert!(
            state_refused(&observed["call_state_usage"][key]),
            "foreign settlement attempt must be refused at {key}: {observed}"
        );
        assert!(
            stop_refused(&observed["live_effect_stop"][key]),
            "foreign session stop must be refused at {key}: {observed}"
        );
    }
    assert!(
        before == after,
        "refusals must leave scoped run/control/events, reservation/call \
         state/usage/binding and result/charge/disposal facts unchanged"
    );

    runs.renew(run).unwrap();
    runs.stop(run).unwrap();
}

#[test]
#[ignore = "runs only inside the dedicated schema fixture worker"]
fn sql_live_aud007_catalog_paths_and_qualification_use_real_catalogs() {
    assert!(worker());
    use duskweave::m1_reasoning::postgres::qualify_owner;
    use duskweave::postgres_m1_terrain::qualify_terrain;

    // Installed metadata for the exact 25 functions: signature, owner, ACL,
    // definer flag and proconfig; catalog-qualified observation only.
    let inventory = altered_function_metadata();
    eprintln!("aud007_functions {}", json!(inventory));

    // Harmless empty catalog-shaped temporary relations inside the caller's
    // own session: denied and passing principals exercise the same boundary.
    fn rust_attempt<E: std::fmt::Debug>(r: Result<(), E>) -> Value {
        match r {
            Ok(()) => json!({"ok": true}),
            Err(e) => json!({"ok": false, "error": format!("{e:?}")}),
        }
    }
    let qualify = |c: &mut postgres::Client, role: &'static str| -> Value {
        json!({
            "qualify_terrain": rust_attempt(qualify_terrain(c)),
            "qualify_owner": rust_attempt(qualify_owner(c, role)),
        })
    };

    let mut denied = terrain_column();
    let denied_meta = session_metadata(&mut denied);
    eprintln!("aud007_session {denied_meta}");
    assert!(denied_meta["temp_eligible"].as_bool().unwrap());
    empty_catalog_shadows(&mut denied);
    let denied_shadowed = qualify(&mut denied, "dw_m1_terrain");
    drop_catalog_shadows(&mut denied);
    let denied_warmed = qualify(&mut denied, "dw_m1_terrain");

    let mut terrain = client("DW_TERRAIN_DATABASE_URL");
    empty_catalog_shadows(&mut terrain);
    let terrain_shadowed = qualify(&mut terrain, "dw_m1_terrain");
    drop_catalog_shadows(&mut terrain);

    let mut broker = broker();
    empty_catalog_shadows(&mut broker);
    let broker_shadowed = json!({
        "qualify_owner": rust_attempt(qualify_owner(&mut broker, "dw_m1_broker")),
    });
    drop_catalog_shadows(&mut broker);

    let observed = json!({
        "denied_shadowed": denied_shadowed,
        "denied_warmed": denied_warmed,
        "terrain_shadowed": terrain_shadowed,
        "broker_shadowed": broker_shadowed,
    });
    eprintln!("aud007_observed {observed}");

    // The denied restricted login must be refused even when every catalog
    // relation its qualification reads is shadowed by an empty temp copy.
    let denied_ok = |v: &Value| v["ok"] == true;
    assert!(
        !denied_ok(&observed["denied_shadowed"]["qualify_terrain"]),
        "qualify_terrain must still reject the denied control under temp catalog shadows: {observed}"
    );
    assert!(
        !denied_ok(&observed["denied_shadowed"]["qualify_owner"]),
        "qualify_owner must still reject the denied control under temp catalog shadows: {observed}"
    );
    assert_eq!(
        observed["denied_shadowed"]["qualify_terrain"]["error"],
        json!("Config(\"unqualified_terrain_role\")")
    );
    assert_eq!(
        observed["denied_shadowed"]["qualify_owner"]["error"],
        json!("\"inference_database_unqualified\"")
    );
    assert!(
        !denied_ok(&observed["denied_warmed"]["qualify_terrain"])
            && !denied_ok(&observed["denied_warmed"]["qualify_owner"]),
        "the warmed-session control must refuse as well: {observed}"
    );

    // Passing controls: the normal Terrain/Broker principals still qualify
    // with identical temporary objects present.
    for (name, v) in [
        (
            "terrain_shadowed.qualify_terrain",
            &observed["terrain_shadowed"]["qualify_terrain"],
        ),
        (
            "terrain_shadowed.qualify_owner",
            &observed["terrain_shadowed"]["qualify_owner"],
        ),
        (
            "broker_shadowed.qualify_owner",
            &observed["broker_shadowed"]["qualify_owner"],
        ),
    ] {
        assert!(
            v["ok"] == true,
            "passing control must qualify at {name}: {observed}"
        );
    }

    // Effective search_path: pg_temp must be listed after pg_catalog on every
    // altered function; an omitted pg_temp still resolves first.
    let unsafe_functions: Vec<&str> = inventory
        .iter()
        .filter(|f| f["safe_relation_resolution"] != true)
        .map(|f| f["signature"].as_str().unwrap())
        .collect();
    assert!(
        unsafe_functions.is_empty(),
        "installed functions without safe pg_temp ordering: {unsafe_functions:?}"
    );
    let definers: Vec<&str> = inventory
        .iter()
        .filter(|f| f["security_definer"] == true)
        .map(|f| f["signature"].as_str().unwrap())
        .collect();
    assert_eq!(definers.len(), 24, "24 definers plus the named invoker");
}
