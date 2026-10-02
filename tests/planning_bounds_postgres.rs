//! Real PostgreSQL tests for v2 scoped/windowed decisions: the producer reads
//! current Mission bounds inside its own SERIALIZABLE transaction, persists
//! the exact nonpositive result, and recovery replays the durable record.

use duskweave::input::parse_register;
use duskweave::mission::{AssetRef, CampaignId, EngagementId, GoalRef, RegistrationInput};
use duskweave::planning::{NonpositiveDecision, PlanningAssessed};
use duskweave::planning_assessment::assess;
use duskweave::planning_history::read_decision;
use duskweave::planning_input::parse_planning;
use duskweave::postgres_mission::{PgAllocator, PgMissionStore};
use duskweave::postgres_trajectory::PgTrajectory;
use duskweave::registration;
use duskweave::{Fail, Res};
use postgres::NoTls;
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

fn client() -> postgres::Client {
    dsn("DW_TEST_DATABASE_URL").connect(NoTls).unwrap()
}

fn admin() -> postgres::Client {
    let mut config = dsn("DW_TEST_ADMIN_DATABASE_URL");
    config.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    config.connect(NoTls).unwrap()
}

fn request_json(e: EngagementId, c: CampaignId, revision: u64, confirmed: bool) -> Value {
    json!({
        "engagement_id": e.0,
        "campaign_id": c.0,
        "purpose_ref": GoalRef(Uuid::from_u128(0x13)),
        "asset_ref": AssetRef(Uuid::from_u128(0x21)),
        "expected_mission_revision": revision,
        "current_authority_confirmed": confirmed,
    })
}

fn assessed(
    allocator: &mut PgAllocator,
    store: &mut PgMissionStore,
    request: &duskweave::planning::PlanningRequest,
) -> Res<Option<PlanningAssessed>> {
    let op = registration::prepare_operation(allocator)?;
    assess(allocator, store, request, op, false)
}

fn reg_with_window(
    e: EngagementId,
    c: duskweave::mission::CampaignId,
    starts_at: i64,
    ends_at: i64,
) -> RegistrationInput {
    let mut body = reg_json(e, c);
    body["starts_at"] = json!(starts_at);
    body["ends_at"] = json!(ends_at);
    parse_register(&serde_json::to_vec(&body).unwrap()).unwrap()
}

fn register_scope(
    allocator: &mut PgAllocator,
    store: &mut PgMissionStore,
    traj: &mut PgTrajectory,
    input: &RegistrationInput,
) {
    let op = registration::prepare_operation(allocator).unwrap();
    registration::register(allocator, store, traj, op, input).unwrap();
}

fn db_epoch() -> i64 {
    client()
        .query_one("SELECT floor(extract(epoch FROM now()))::bigint", &[])
        .unwrap()
        .get(0)
}

#[test]
fn scoped_current_basis_decisions_persist_and_match_owner_row() {
    let _guard = db();
    let (e, c) = scope(0xb2a0);
    let mut allocator = PgAllocator::new(client());
    let mut store = PgMissionStore::new(client());
    let mut traj = PgTrajectory::new(client());
    let now = db_epoch();
    register_scope(
        &mut allocator,
        &mut store,
        &mut traj,
        &reg_with_window(e, c, now - 7_200, now + 7_200),
    );
    let input = parse_planning(&serde_json::to_vec(&request_json(e, c, 1, true)).unwrap()).unwrap();
    let event = assessed(&mut allocator, &mut store, &input)
        .unwrap()
        .unwrap();
    assert_eq!(event.version, 2);
    assert_eq!(
        event.decision,
        NonpositiveDecision::UnresolvedEvaluationIncomplete
    );
    let scope = event.basis.as_ref().unwrap().scope.as_ref().unwrap();
    let row = client()
        .query_one(
            "SELECT goal_ref, included_assets, excluded_assets FROM mission.missions \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap();
    assert_eq!(scope.goal_ref.0, row.get::<_, Uuid>(0));
    let included: Vec<Uuid> = serde_json::from_value(row.get::<_, Value>(1)).unwrap();
    let excluded: Vec<Uuid> = serde_json::from_value(row.get::<_, Value>(2)).unwrap();
    assert_eq!(
        scope
            .included_assets
            .iter()
            .map(|a| a.0)
            .collect::<Vec<_>>(),
        included
    );
    assert_eq!(
        scope
            .excluded_assets
            .iter()
            .map(|a| a.0)
            .collect::<Vec<_>>(),
        excluded
    );
    let basis = event.basis.as_ref().unwrap();
    assert!(basis.starts_at <= event.evaluated_at);
    assert!(event.evaluated_at < basis.ends_at);
}

#[test]
fn purpose_asset_and_window_refusals_are_durable() {
    let _guard = db();
    let (e, c) = scope(0xb2b0);
    let mut allocator = PgAllocator::new(client());
    let mut store = PgMissionStore::new(client());
    let mut traj = PgTrajectory::new(client());
    let now = db_epoch();
    register_scope(
        &mut allocator,
        &mut store,
        &mut traj,
        &reg_with_window(e, c, now - 7_200, now + 7_200),
    );
    let mut bad_purpose = request_json(e, c, 1, true);
    bad_purpose["purpose_ref"] = json!(Uuid::from_u128(0xb2b9));
    let mut excluded = request_json(e, c, 1, true);
    excluded["asset_ref"] = json!(Uuid::from_u128(0x23));
    let mut unknown = request_json(e, c, 1, true);
    unknown["asset_ref"] = json!(Uuid::from_u128(0xb2ba));
    let mut bad_revision = request_json(e, c, 2, true);
    bad_revision["asset_ref"] = json!(Uuid::from_u128(0x23));
    let mut unconfirmed = request_json(e, c, 1, false);
    unconfirmed["purpose_ref"] = json!(Uuid::from_u128(0xb2b9));
    for (body, expected) in [
        (bad_purpose, NonpositiveDecision::RefusedPurposeMismatch),
        (excluded, NonpositiveDecision::RefusedAssetExcluded),
        (unknown, NonpositiveDecision::RefusedAssetUnknown),
        (bad_revision, NonpositiveDecision::RefusedRevisionMismatch),
        (
            unconfirmed,
            NonpositiveDecision::UnresolvedAuthorityUnconfirmed,
        ),
    ] {
        let input = parse_planning(&serde_json::to_vec(&body).unwrap()).unwrap();
        let event = assessed(&mut allocator, &mut store, &input)
            .unwrap()
            .unwrap();
        assert_eq!(event.decision, expected);
        assert_eq!(event.version, 2);
    }
    // Window refusals come from the owner's validity interval, not the caller.
    let (e2, c2) = scope(0xb2c0);
    register_scope(
        &mut allocator,
        &mut store,
        &mut traj,
        &reg_with_window(e2, c2, now + 7_200, now + 14_400),
    );
    let input =
        parse_planning(&serde_json::to_vec(&request_json(e2, c2, 1, true)).unwrap()).unwrap();
    let event = assessed(&mut allocator, &mut store, &input)
        .unwrap()
        .unwrap();
    assert_eq!(event.decision, NonpositiveDecision::RefusedNotYetValid);
    assert!(event.evaluated_at < event.basis.as_ref().unwrap().starts_at);
    let (e3, c3) = scope(0xb2c8);
    register_scope(
        &mut allocator,
        &mut store,
        &mut traj,
        &reg_with_window(e3, c3, now - 14_400, now - 7_200),
    );
    let input =
        parse_planning(&serde_json::to_vec(&request_json(e3, c3, 1, true)).unwrap()).unwrap();
    let event = assessed(&mut allocator, &mut store, &input)
        .unwrap()
        .unwrap();
    assert_eq!(event.decision, NonpositiveDecision::RefusedExpired);
    assert!(event.evaluated_at >= event.basis.as_ref().unwrap().ends_at);
}

#[test]
fn recovery_returns_original_after_owner_removed_or_changed() {
    let _guard = db();
    let (e, c) = scope(0xb2d0);
    let mut allocator = PgAllocator::new(client());
    let mut store = PgMissionStore::new(client());
    let mut traj = PgTrajectory::new(client());
    let now = db_epoch();
    register_scope(
        &mut allocator,
        &mut store,
        &mut traj,
        &reg_with_window(e, c, now - 7_200, now + 7_200),
    );
    let input = parse_planning(&serde_json::to_vec(&request_json(e, c, 1, true)).unwrap()).unwrap();
    let op = registration::prepare_operation(&mut allocator).unwrap();
    let first = assess(&mut allocator, &mut store, &input, op, false)
        .unwrap()
        .unwrap();
    assert_eq!(
        first.decision,
        NonpositiveDecision::UnresolvedEvaluationIncomplete
    );
    // Changing owner bounds later does not rewrite the durable record.
    admin()
        .execute(
            "UPDATE mission.missions SET goal_ref=$3 \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0, &Uuid::from_u128(0xb2d9)],
        )
        .unwrap();
    let recovered = read_decision(&mut store, &input, op).unwrap().unwrap();
    assert_eq!(recovered, first);
    // A genuinely new operation reads the CURRENT owner values.
    let second = assessed(&mut allocator, &mut store, &input)
        .unwrap()
        .unwrap();
    assert_eq!(second.decision, NonpositiveDecision::RefusedPurposeMismatch);
    assert_eq!(
        second
            .basis
            .as_ref()
            .unwrap()
            .scope
            .as_ref()
            .unwrap()
            .goal_ref
            .0,
        Uuid::from_u128(0xb2d9)
    );
    // Removing the owner row still leaves recovery deterministic.
    admin()
        .execute(
            "DELETE FROM mission.missions WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap();
    let recovered = read_decision(&mut store, &input, op).unwrap().unwrap();
    assert_eq!(recovered, first);
    assert_eq!(recovered.assessment_labels(), ("matched", "within_window"));
}

#[test]
fn malformed_owner_scope_fails_without_any_effect() {
    let _guard = db();
    let (e, c) = scope(0xb2e8);
    let mut allocator = PgAllocator::new(client());
    let mut store = PgMissionStore::new(client());
    let mut traj = PgTrajectory::new(client());
    let now = db_epoch();
    register_scope(
        &mut allocator,
        &mut store,
        &mut traj,
        &reg_with_window(e, c, now - 7_200, now + 7_200),
    );
    admin()
        .execute(
            "UPDATE mission.missions SET included_assets='[]'::jsonb \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap();
    let input = parse_planning(&serde_json::to_vec(&request_json(e, c, 1, true)).unwrap()).unwrap();
    let op = registration::prepare_operation(&mut allocator).unwrap();
    assert_eq!(
        assess(&mut allocator, &mut store, &input, op, false),
        Err(Fail::Store("unsupported_basis"))
    );
    let counts: i64 = client()
        .query_one(
            "SELECT count(*) FROM mission.planning_assessments \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap()
        .get(0);
    assert_eq!(counts, 0);
}

#[test]
fn identical_references_under_another_campaign_get_no_basis_or_history() {
    let _guard = db();
    let (e, c) = scope(0xb310);
    let (_, c_other) = scope(0xb314);
    let mut allocator = PgAllocator::new(client());
    let mut store = PgMissionStore::new(client());
    let mut traj = PgTrajectory::new(client());
    let now = db_epoch();
    register_scope(
        &mut allocator,
        &mut store,
        &mut traj,
        &reg_with_window(e, c, now - 7_200, now + 7_200),
    );
    // The identical request fields under a different campaign read no basis.
    let mut body = request_json(e, c_other, 1, true);
    body["engagement_id"] = json!(e.0);
    let input = parse_planning(&serde_json::to_vec(&body).unwrap()).unwrap();
    let op = registration::prepare_operation(&mut allocator).unwrap();
    let event = assess(&mut allocator, &mut store, &input, op, false)
        .unwrap()
        .unwrap();
    assert_eq!(event.decision, NonpositiveDecision::UnresolvedMissionBasis);
    assert!(event.basis.is_none());
    // Reusing the identical operation handle under the registered campaign
    // produces an independent scoped record, never the other scope's result.
    let input = parse_planning(&serde_json::to_vec(&request_json(e, c, 1, true)).unwrap()).unwrap();
    let original = assess(&mut allocator, &mut store, &input, op, false)
        .unwrap()
        .unwrap();
    assert_eq!(
        original.decision,
        NonpositiveDecision::UnresolvedEvaluationIncomplete
    );
    assert!(original.basis.is_some());
    assert_ne!(original.campaign_id, event.campaign_id);
}
