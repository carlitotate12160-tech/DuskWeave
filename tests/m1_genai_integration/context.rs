//! SDK context qualification through the existing native fixture.
use super::*;

pub(super) fn sdk_context_reaches_real_owner_pack_before_reservation() {
    let _guard = native::db();
    refresh();
    let q = registered(0x2a101);
    let record = run_support::start_only(&q);
    let server = fixture::Server::start("127.0.0.1:18083", |_| fixture::Reply::Reset);
    let before = owner_counts(record.decision_one);
    assert_eq!(before, [0, 0, 0]);
    // Existing helper crosses run lease/phase, owner views and real ContextPack.
    // It must succeed without inventing any future generation or call identity.
    let preparation_started = std::time::Instant::now();
    let prepared = lifecycle::prepare(&record, duskweave::m1_run::RunPhase::DecisionOne);
    eprintln!(
        "safe_sdk_preparation stage=owner_context_preflight elapsed_ms={} reservations=0 calls=0 sends={}",
        preparation_started.elapsed().as_millis(),
        server.count()
    );
    let p: serde_json::Value = serde_json::from_str(prepared.pack.utf8()).unwrap();
    assert_eq!(p["version"], "m1_context_pack_autonomous_v3");
    assert_eq!(owner_counts(record.decision_one), before);
    assert_eq!(
        server.count(),
        0,
        "real ContextPack preflight must not send"
    );
    capacity_boundaries(prepared.pack.utf8().as_bytes());
    use duskweave::m1_reasoning::{inference::Broker, inference_intent as intent};
    let mut broker = lifecycle::broker(&record, &prepared);
    let reserved = broker
        .reserve(&prepared.scope, prepared.pack.utf8().as_bytes())
        .unwrap();
    let upper = intent::preflight(prepared.pack.utf8().as_bytes(), 8192, 16384).unwrap();
    for (call, codes) in [
        (reserved.generation_call, None),
        (
            reserved.repair_call,
            Some(&["unexpected_view_request"; 8][..]),
        ),
    ] {
        let bound = intent::bind(
            &prepared.scope,
            &reserved,
            call,
            prepared.pack.utf8().as_bytes(),
            codes,
        )
        .unwrap();
        assert!(bound.configuration.len() <= upper);
        let i: serde_json::Value = serde_json::from_slice(&bound.configuration).unwrap();
        assert_eq!(i["generation"], reserved.generation);
        assert_eq!(
            i["generation_call_id"],
            reserved.generation_call.to_string()
        );
        assert_eq!(i["repair_call_id"], reserved.repair_call.to_string());
        assert_eq!(i["call_id"], call.to_string());
        if codes.is_none() {
            assert_rehashed_tampering_refused(&mut broker, &prepared.scope, &reserved, &bound);
        }
    }
    assert_eq!(
        server.count(),
        0,
        "binding actual reservation must not send"
    );
}
fn capacity_boundaries(pack: &[u8]) {
    use duskweave::m1_reasoning::{inference_intent as intent, request};
    let upper = intent::preflight(pack, pack.len(), 16384).unwrap();
    assert!(intent::preflight(pack, pack.len() - 1, 16384).is_err());
    assert_eq!(intent::preflight(pack, 8192, upper).unwrap(), upper);
    assert!(intent::preflight(pack, 8192, upper - 1).is_err());
    assert!(
        request::bind(pack, None).is_err(),
        "pre-reservation vendor binding stays refused"
    );
    let mut exact = pack.to_vec();
    exact.resize(8192, b' ');
    assert!(intent::preflight(&exact, 8192, 16384).is_ok());
    exact.push(b' ');
    assert!(intent::preflight(&exact, 8192, 16384).is_err());
    let original: serde_json::Value = serde_json::from_slice(pack).unwrap();
    for text in ["\"\\".repeat(200), "\u{754c}".repeat(200)] {
        let mut p = original.clone();
        p["question"] = serde_json::json!(text);
        assert!(intent::preflight(&serde_json::to_vec(&p).unwrap(), 8192, 16384).is_ok());
    }
    for (key, value) in [
        ("version", "m1_context_pack_autonomous_v2"),
        ("inference_profile", "fixture-responses-v1"),
        ("execution_mode", "live"),
    ] {
        let mut p = original.clone();
        p[key] = serde_json::json!(value);
        assert!(intent::preflight(&serde_json::to_vec(&p).unwrap(), 8192, 16384).is_err());
    }
}

fn assert_rehashed_tampering_refused(
    broker: &mut duskweave::m1_reasoning::postgres::PgBroker,
    scope: &duskweave::m1_reasoning::inference::Scope,
    reserved: &duskweave::m1_reasoning::inference::Reservation,
    original: &duskweave::m1_reasoning::request::BoundCall,
) {
    use duskweave::m1_reasoning::{
        inference::Broker, inference_intent as intent, request::BoundCall,
    };
    let before = owner_counts(scope.decision);
    for (field, value) in [
        ("recipient", "http://127.0.0.1:18084/responses"),
        ("model", "unapproved"),
        ("permission_id", "00000000-0000-0000-0000-000000000099"),
        ("campaign_id", "00000000-0000-0000-0000-000000000099"),
        ("call_role", "repair"),
        ("generation", "2"),
        ("settings", "changed"),
    ] {
        let mut i: serde_json::Value = serde_json::from_slice(&original.configuration).unwrap();
        match field {
            "settings" => i["settings"]["store"] = serde_json::json!(true),
            "generation" => i[field] = serde_json::json!(reserved.generation + 1),
            _ => i[field] = serde_json::json!(value),
        }
        i.sort_all_objects();
        let configuration = serde_json::to_vec(&i).unwrap();
        let tampered = BoundCall {
            pack: original.pack.clone(),
            body: Vec::new(),
            digest: intent::digest(&original.pack, &configuration),
            configuration,
        };
        assert!(
            intent::verify_owned(&tampered, scope, reserved, reserved.generation_call).is_err(),
            "Rust must refuse recomputed {field} tampering"
        );
        assert!(
            broker
                .may_send(scope, reserved.generation_call, &tampered)
                .is_err(),
            "restricted SQL must refuse recomputed {field} tampering"
        );
        assert_eq!(
            owner_counts(scope.decision),
            before,
            "invalid admission cannot retain a marker"
        );
    }
}
