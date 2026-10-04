//! Journal regression through the real PgTrajectory ports: canonical typed
//! contract equality for v1/v2 records regardless of wire key order or an
//! explicit null optional field, plus bounded store failures on catalog
//! corruption — never absence, false conflict or completion.
use duskweave::Fail;
use duskweave::mission::{CampaignId, EngagementId};
use duskweave::planning::PlanningAssessed;
use duskweave::planning_history::PlanningHistoryPort;
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration;
use duskweave::trajectory::Delivered;
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/planning_history_db.rs"]
mod history_support;
use db_support::*;
use history_support::*;

/// Renders JSON text with every object's keys in reverse order; semantic
/// identity never depends on wire key order.
fn rekey(value: &Value) -> String {
    match value {
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .rev()
                .map(|(k, v)| format!("{}:{}", json!(k), rekey(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(rekey).collect::<Vec<_>>().join(",")
        ),
        _ => value.to_string(),
    }
}

/// Legacy v1 event JSON; production has no v1 constructor. The basis binds to
/// the accepted registration predecessor's operation and validity interval.
fn v1_event(
    e: EngagementId,
    c: CampaignId,
    operation: Uuid,
    event: Uuid,
    registration_operation: Uuid,
    window: (i64, i64),
    at: i64,
) -> Value {
    json!({
        "event_id": event, "operation_id": operation,
        "engagement_id": e.0, "campaign_id": c.0,
        "producer": "mission", "kind": "planning_assessed", "version": 1,
        "affected_entity": operation, "owner_revision": 1,
        "causation_id": operation, "correlation_id": operation,
        "request": {
            "engagement_id": e.0, "campaign_id": c.0,
            "purpose_ref": Uuid::from_u128(0x13),
            "asset_ref": Uuid::from_u128(0x21),
            "expected_mission_revision": 1,
            "current_authority_confirmed": true
        },
        "basis": {
            "registration_operation_id": registration_operation, "revision": 1,
            "exercise_mode": "blind", "starts_at": window.0, "ends_at": window.1
        },
        "decision": "unresolved_evaluation_incomplete",
        "evaluated_at": at, "occurred_at": at, "recorded_at": at,
        "time_basis": "producer_transaction_start"
    })
}

#[test]
fn v1_semantic_identity_tolerates_null_scope_and_reordered_keys() {
    let _guard = db();
    let (e, c) = scope(0xb3a0);
    let (mut alloc, mut store, mut trajectory) = ports();
    let registration_operation = registration::prepare_operation(&mut alloc).unwrap();
    registration::register(
        &mut alloc,
        &mut store,
        &mut trajectory,
        registration_operation,
        &reg_input(e, c),
    )
    .unwrap();
    let original = v1_event(
        e,
        c,
        Uuid::from_u128(0xb3a1),
        Uuid::from_u128(0xb3a2),
        registration_operation.0,
        (1_700_000_000, 1_700_086_400),
        1_700_010_000,
    );
    let event: PlanningAssessed = serde_json::from_value(original).unwrap();
    assert_eq!(event.version, 1);
    assert!(event.basis.as_ref().unwrap().scope.is_none());
    let mut consumer = PgTrajectory::new(runtime_client());
    assert_eq!(consumer.publish(&event).unwrap(), Delivered::Completed);

    // Same typed record decoded from explicit null scope and reversed keys.
    let mut variant = serde_json::to_value(&event).unwrap();
    variant["basis"]["scope"] = Value::Null;
    let decoded: PlanningAssessed = serde_json::from_str(&rekey(&variant)).unwrap();
    assert_eq!(decoded, event);
    assert_eq!(consumer.inspect(&decoded).unwrap(), Delivered::Completed);
    assert_eq!(consumer.publish(&decoded).unwrap(), Delivered::Duplicate);

    // The original row kept its exact contract, version, identities and times.
    let mut fresh = PgTrajectory::new(runtime_client());
    assert_eq!(fresh.inspect(&event).unwrap(), Delivered::Completed);
    let row = runtime_client()
        .query_one(
            "SELECT contract, version, event_id, operation_id, recorded_at = completed_at \
             FROM trajectory.planning_history \
             WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e.0, &c.0],
        )
        .unwrap();
    let contract: Value = row.get(0);
    assert_eq!(contract, serde_json::to_value(&event).unwrap());
    assert!(contract["basis"].get("scope").is_none());
    assert_eq!(row.get::<_, i32>(1), 1);
    assert_eq!(row.get::<_, Uuid>(2), event.event_id.0);
    assert_eq!(row.get::<_, Uuid>(3), event.operation_id.0);
    assert!(row.get::<_, bool>(4));
    assert_eq!(count("trajectory.planning_history", e, c), 1);
}

#[test]
fn v2_semantic_identity_tolerates_reordered_nested_keys() {
    let _guard = db();
    let event = decision(0xb3b0, true);
    assert_eq!(event.version, 2);
    let mut consumer = PgTrajectory::new(runtime_client());
    assert_eq!(
        retry(|| consumer.publish(&event)).unwrap(),
        Delivered::Completed
    );
    let reordered: PlanningAssessed =
        serde_json::from_str(&rekey(&serde_json::to_value(&event).unwrap())).unwrap();
    assert_eq!(reordered, event);
    assert_eq!(consumer.inspect(&reordered).unwrap(), Delivered::Completed);
    // The full accepted row stays byte-identical across duplicate delivery.
    let accepted_row = || {
        runtime_client()
            .query_one(
                "SELECT to_jsonb(h) FROM trajectory.planning_history h \
                 WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
                &[&event.engagement_id.0, &event.campaign_id.0],
            )
            .unwrap()
            .get::<_, Value>(0)
    };
    let before = accepted_row();
    assert_eq!(consumer.publish(&reordered).unwrap(), Delivered::Duplicate);
    assert_eq!(accepted_row(), before);
    effects(&event, 1, 0);
}

fn patch_contract(e: EngagementId, c: CampaignId, path: &str, value: &str) {
    let patch: Value = serde_json::from_str(value).unwrap();
    admin()
        .execute(
            &format!(
                "UPDATE trajectory.planning_history \
                 SET contract=jsonb_set(contract,'{path}',$3) \
                 WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'"
            ),
            &[&e.0, &c.0, &patch],
        )
        .unwrap();
}

#[test]
fn stored_corruption_is_bounded_failure_never_absence_or_conflict() {
    let _guard = db();
    let event = decision(0xb3c0, true);
    let (e, c) = (event.engagement_id, event.campaign_id);
    let mut consumer = PgTrajectory::new(runtime_client());
    assert_eq!(
        retry(|| consumer.publish(&event)).unwrap(),
        Delivered::Completed
    );
    let original: Value = admin()
        .query_one(
            "SELECT contract FROM trajectory.planning_history \
             WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e.0, &c.0],
        )
        .unwrap()
        .get(0);

    // Malformed typed-contract variants keep their bounded categories.
    for (path, value, expected) in [
        ("{surprise}", "1", "contract_decode"),
        ("{version}", "99", "contract_decode"),
        (
            "{decision}",
            "\"unresolved_evaluation_incomplete\"",
            "contract_decode",
        ),
        ("{occurred_at}", "1", "contract_decode"),
        ("{basis,starts_at}", "99999999999", "unsupported_basis"),
    ] {
        patch_contract(e, c, path, value);
        assert_eq!(consumer.inspect(&event), Err(Fail::Store(expected)));
        assert_eq!(consumer.publish(&event), Err(Fail::Store(expected)));
        assert_eq!(count("trajectory.planning_history", e, c), 1);
        admin()
            .execute(
                "UPDATE trajectory.planning_history SET contract=$3 \
                 WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
                &[&e.0, &c.0, &original],
            )
            .unwrap();
    }

    // A wholly malformed stored contract decodes as corruption, not absence.
    admin()
        .execute(
            "UPDATE trajectory.planning_history SET contract='{}'::jsonb \
             WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e.0, &c.0],
        )
        .unwrap();
    assert_eq!(
        consumer.inspect(&event),
        Err(Fail::Store("contract_decode"))
    );
    admin()
        .execute(
            "UPDATE trajectory.planning_history SET contract=$3 \
             WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e.0, &c.0, &original],
        )
        .unwrap();

    // A catalog column disagreeing with the validated record's header is a
    // bounded store failure; the supported version pair exercises it.
    admin()
        .execute(
            "UPDATE trajectory.planning_history SET version=1 \
             WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e.0, &c.0],
        )
        .unwrap();
    assert_eq!(
        consumer.inspect(&event),
        Err(Fail::Store("contract_decode"))
    );
    assert_eq!(
        consumer.publish(&event),
        Err(Fail::Store("contract_decode"))
    );
    admin()
        .execute(
            "UPDATE trajectory.planning_history SET version=$3 \
             WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e.0, &c.0, &(event.version as i32)],
        )
        .unwrap();

    // Catalog corruption on either identity axis is still a bounded failure:
    // the surviving OR axis finds the row and the provenance comparison
    // rejects the disagreement without new effects.
    for (column, corrupt, original_col) in [
        ("event_id", Uuid::from_u128(0xb3cc), event.event_id.0),
        (
            "operation_id",
            Uuid::from_u128(0xb3cd),
            event.operation_id.0,
        ),
    ] {
        admin()
            .execute(
                &format!(
                    "UPDATE trajectory.planning_history SET {column}=$3 \
                     WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'"
                ),
                &[&e.0, &c.0, &corrupt],
            )
            .unwrap();
        assert_eq!(
            consumer.inspect(&event),
            Err(Fail::Store("contract_decode")),
            "{column}"
        );
        assert_eq!(
            consumer.publish(&event),
            Err(Fail::Store("contract_decode")),
            "{column}"
        );
        effects(&event, 1, 0);
        admin()
            .execute(
                &format!(
                    "UPDATE trajectory.planning_history SET {column}=$3 \
                     WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'"
                ),
                &[&e.0, &c.0, &original_col],
            )
            .unwrap();
    }

    // A standalone-valid stored contract disagreeing with the validated
    // record's scope on either axis is rejected by the same catalog binding.
    for (top, nested) in [
        ("{engagement_id}", "{request,engagement_id}"),
        ("{campaign_id}", "{request,campaign_id}"),
    ] {
        let stray = format!("\"{}\"", Uuid::from_u128(0xb3ce));
        patch_contract(e, c, top, &stray);
        patch_contract(e, c, nested, &stray);
        assert_eq!(
            consumer.inspect(&event),
            Err(Fail::Store("contract_decode")),
            "{top}"
        );
        assert_eq!(
            consumer.publish(&event),
            Err(Fail::Store("contract_decode")),
            "{top}"
        );
        effects(&event, 1, 0);
        admin()
            .execute(
                "UPDATE trajectory.planning_history SET contract=$3 \
                 WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
                &[&e.0, &c.0, &original],
            )
            .unwrap();
    }

    let mut fresh = PgTrajectory::new(runtime_client());
    assert_eq!(fresh.inspect(&event).unwrap(), Delivered::Completed);
    assert_eq!(fresh.publish(&event).unwrap(), Delivered::Duplicate);
    effects(&event, 1, 0);
}
