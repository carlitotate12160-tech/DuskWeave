use duskweave::Fail;
use duskweave::mission::{AssetRef, CampaignId, EngagementId, GoalRef, OperationId};
use duskweave::planning::{NonpositiveDecision, PlanningRequest};
use duskweave::planning_assessment;
use duskweave::registration::{self, MissionStore, OperationAllocator};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
use db_support::*;

fn request(engagement: EngagementId, campaign: CampaignId) -> PlanningRequest {
    PlanningRequest {
        engagement_id: engagement,
        campaign_id: campaign,
        purpose_ref: GoalRef(Uuid::from_u128(0x13)),
        asset_ref: AssetRef(Uuid::from_u128(0x21)),
        expected_mission_revision: 1,
        current_authority_confirmed: true,
    }
}

fn admin() -> postgres::Client {
    let mut config = dsn("DW_TEST_ADMIN_DATABASE_URL");
    config.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    config.connect(postgres::NoTls).unwrap()
}

fn assessment_row(e: EngagementId, c: CampaignId, op: OperationId) -> serde_json::Value {
    runtime_client()
        .query_one(
            "SELECT to_jsonb(a) FROM mission.planning_assessments a \
             WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
            &[&e.0, &c.0, &op.0],
        )
        .unwrap()
        .get(0)
}

fn owner_row(e: EngagementId, c: CampaignId) -> serde_json::Value {
    runtime_client()
        .query_one(
            "SELECT to_jsonb(m) FROM mission.missions m \
             WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0],
        )
        .unwrap()
        .get(0)
}

struct NeverAllocate;

struct FixedAllocator(Uuid);

impl OperationAllocator for FixedAllocator {
    fn allocate(&mut self) -> duskweave::Res<Uuid> {
        Ok(self.0)
    }
}

impl OperationAllocator for NeverAllocate {
    fn allocate(&mut self) -> duskweave::Res<Uuid> {
        panic!("duplicate/recovery allocated event")
    }
}

#[test]
fn durable_decision_duplicate_and_recovery_keep_original() {
    let _guard = db();
    let (engagement, campaign) = scope(0xb1a1);
    let (mut allocator, mut store, mut trajectory) = ports();
    let registration = accepted(
        &mut allocator,
        &mut store,
        &mut trajectory,
        engagement,
        campaign,
    );
    let operation = registration::prepare_operation(&mut allocator).unwrap();
    let input = request(engagement, campaign);
    let first = planning_assessment::assess(&mut allocator, &mut store, &input, operation, false)
        .unwrap()
        .expect("durable decision");
    assert_eq!(first.decision, NonpositiveDecision::RefusedExpired);
    assert_eq!(first.version, 2);
    assert_eq!(
        first.basis.as_ref().unwrap().registration_operation_id,
        registration.operation_id
    );
    assert_eq!(
        count("mission.planning_assessments", engagement, campaign),
        1
    );
    assert_eq!(
        count("trajectory.registration_history", engagement, campaign),
        1
    );
    let obligation: String = runtime_client()
        .query_one(
            "SELECT publication_obligation FROM mission.planning_assessments \
         WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
            &[&engagement.0, &campaign.0, &operation.0],
        )
        .unwrap()
        .get(0);
    assert_eq!(obligation, "trajectory.planning_history.v1");
    let recovered =
        planning_assessment::assess(&mut NeverAllocate, &mut store, &input, operation, true)
            .unwrap();
    assert_eq!(recovered, Some(first.clone()));
    assert_eq!(
        planning_assessment::assess(&mut NeverAllocate, &mut store, &input, operation, false)
            .unwrap(),
        Some(first)
    );
    let mut changed = input;
    changed.current_authority_confirmed = false;
    assert_eq!(
        planning_assessment::assess(&mut allocator, &mut store, &changed, operation, true),
        Err(Fail::Conflict("integrity_conflict"))
    );
    assert_eq!(
        store
            .mission_view(engagement, campaign)
            .unwrap()
            .unwrap()
            .revision,
        1
    );
}

#[test]
fn all_intent_fields_conflict_and_scopes_are_independent() {
    let _guard = db();
    let (engagement, campaign) = scope(0xb1a3);
    let (mut allocator, mut store, _trajectory) = ports();
    let operation = OperationId(allocator.allocate().unwrap());
    let input = request(engagement, campaign);
    let event_id = Uuid::from_u128(0xb1a3);
    let original = planning_assessment::assess(
        &mut FixedAllocator(event_id),
        &mut store,
        &input,
        operation,
        false,
    )
    .unwrap()
    .unwrap();
    let mut variations = Vec::new();
    let mut changed = input.clone();
    changed.purpose_ref = GoalRef(Uuid::from_u128(88));
    variations.push(changed);
    let mut changed = input.clone();
    changed.asset_ref = AssetRef(Uuid::from_u128(89));
    variations.push(changed);
    let mut changed = input.clone();
    changed.expected_mission_revision = 2;
    variations.push(changed);
    let mut changed = input.clone();
    changed.current_authority_confirmed = false;
    variations.push(changed);
    for changed in variations {
        assert_eq!(
            planning_assessment::assess(&mut NeverAllocate, &mut store, &changed, operation, false),
            Err(Fail::Conflict("integrity_conflict"))
        );
    }
    let (other_engagement, other_campaign) = scope(0xb1a4);
    let other = request(other_engagement, other_campaign);
    let separate = planning_assessment::assess(
        &mut FixedAllocator(event_id),
        &mut store,
        &other,
        operation,
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(separate.operation_id, original.operation_id);
    assert_eq!(separate.event_id, original.event_id);
    assert_ne!(separate.engagement_id, original.engagement_id);
    for (engagement_id, campaign_id) in [(engagement, other_campaign), (other_engagement, campaign)]
    {
        let independent = planning_assessment::assess(
            &mut FixedAllocator(event_id),
            &mut store,
            &request(engagement_id, campaign_id),
            operation,
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(independent.event_id, original.event_id);
        assert_eq!(
            count("mission.planning_assessments", engagement_id, campaign_id),
            1
        );
    }
    assert_eq!(
        count("mission.planning_assessments", engagement, campaign),
        1
    );
    assert_eq!(
        count(
            "mission.planning_assessments",
            other_engagement,
            other_campaign
        ),
        1
    );
}

#[test]
fn cross_family_identity_collision_rejects_both_directions() {
    let _guard = db();
    let (engagement, campaign) = scope(0xb1a5);
    let (mut allocator, mut store, mut trajectory) = ports();
    let registered = accepted(
        &mut allocator,
        &mut store,
        &mut trajectory,
        engagement,
        campaign,
    );
    assert_eq!(
        planning_assessment::assess(
            &mut NeverAllocate,
            &mut store,
            &request(engagement, campaign),
            registered.operation_id,
            false
        ),
        Err(Fail::Conflict("integrity_conflict"))
    );
    let (engagement, campaign) = scope(0xb1a6);
    let operation = OperationId(allocator.allocate().unwrap());
    planning_assessment::assess(
        &mut allocator,
        &mut store,
        &request(engagement, campaign),
        operation,
        false,
    )
    .unwrap();
    assert_eq!(
        reg(
            &mut allocator,
            &mut store,
            &mut trajectory,
            operation,
            engagement,
            campaign
        ),
        Err(Fail::Conflict("integrity_conflict"))
    );
    assert_eq!(
        count("mission.registration_outbox", engagement, campaign),
        0
    );
}

#[test]
fn invalid_owner_basis_is_not_invented_absence() {
    let _guard = db();
    let (engagement, campaign) = scope(0xb1a7);
    let (mut allocator, mut store, mut trajectory) = ports();
    let registration = accepted(
        &mut allocator,
        &mut store,
        &mut trajectory,
        engagement,
        campaign,
    );
    let original_row = owner_row(engagement, campaign);
    admin()
        .execute(
            "UPDATE mission.missions SET operation_id=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&engagement.0, &campaign.0, &Uuid::nil()],
        )
        .unwrap();
    let corrupted = owner_row(engagement, campaign);
    let operation = OperationId(allocator.allocate().unwrap());
    assert_eq!(
        planning_assessment::assess(
            &mut NeverAllocate,
            &mut store,
            &request(engagement, campaign),
            operation,
            false
        ),
        Err(Fail::Store("unsupported_basis"))
    );
    assert_eq!(owner_row(engagement, campaign), corrupted);
    assert_eq!(
        count("mission.planning_assessments", engagement, campaign),
        0
    );
    admin()
        .execute(
            "UPDATE mission.missions SET operation_id=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&engagement.0, &campaign.0, &registration.operation_id.0],
        )
        .unwrap();
    assert_eq!(owner_row(engagement, campaign), original_row);
    assert!(
        planning_assessment::assess(
            &mut allocator,
            &mut store,
            &request(engagement, campaign),
            operation,
            false
        )
        .unwrap()
        .is_some()
    );
}

#[test]
fn corrupt_stored_contract_is_error_not_not_committed() {
    let _guard = db();
    let (engagement, campaign) = scope(0xb1ae);
    let (mut allocator, mut store, _trajectory) = ports();
    let operation = OperationId(allocator.allocate().unwrap());
    let input = request(engagement, campaign);
    let original =
        planning_assessment::assess(&mut allocator, &mut store, &input, operation, false)
            .unwrap()
            .expect("durable decision");
    let original_row = assessment_row(engagement, campaign, operation);
    admin()
        .execute(
            "UPDATE mission.planning_assessments SET contract='{}'::jsonb \
         WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
            &[&engagement.0, &campaign.0, &operation.0],
        )
        .unwrap();
    let corrupted = assessment_row(engagement, campaign, operation);
    for recover in [true, false] {
        assert_eq!(
            planning_assessment::assess(&mut NeverAllocate, &mut store, &input, operation, recover),
            Err(Fail::Store("contract_decode"))
        );
        assert_eq!(assessment_row(engagement, campaign, operation), corrupted);
    }
    admin()
        .execute(
            "UPDATE mission.planning_assessments SET contract=$4 \
         WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
            &[
                &engagement.0,
                &campaign.0,
                &operation.0,
                &original_row["contract"],
            ],
        )
        .unwrap();
    assert_eq!(
        assessment_row(engagement, campaign, operation),
        original_row
    );
    assert_eq!(
        planning_assessment::assess(&mut NeverAllocate, &mut store, &input, operation, true)
            .unwrap(),
        Some(original)
    );
    assert_eq!(
        count("mission.planning_assessments", engagement, campaign),
        1
    );
}

#[test]
fn stored_corruption_fails_in_validation_then_binding_order() {
    let _guard = db();
    let (engagement, campaign) = scope(0xb1c0);
    let (mut allocator, mut store, mut trajectory) = ports();
    accepted(
        &mut allocator,
        &mut store,
        &mut trajectory,
        engagement,
        campaign,
    );
    let operation = OperationId(allocator.allocate().unwrap());
    let input = request(engagement, campaign);
    let original =
        planning_assessment::assess(&mut allocator, &mut store, &input, operation, false)
            .unwrap()
            .expect("durable decision");
    let original_row = assessment_row(engagement, campaign, operation);

    // The registered mission gives the stored event a real typed basis.
    // Catalog mismatch: the typed payload is valid but the stored event
    // identity no longer binds to it.
    admin()
        .execute(
            "UPDATE mission.planning_assessments SET event_id=$4 \
         WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
            &[
                &engagement.0,
                &campaign.0,
                &operation.0,
                &Uuid::from_u128(0xb1c1),
            ],
        )
        .unwrap();
    let corrupted = assessment_row(engagement, campaign, operation);
    for recover in [true, false] {
        assert_eq!(
            planning_assessment::assess(&mut NeverAllocate, &mut store, &input, operation, recover),
            Err(Fail::Store("contract_decode"))
        );
        assert_eq!(assessment_row(engagement, campaign, operation), corrupted);
    }

    // A typed payload whose stored basis is invalid stays unsupported_basis;
    // combining it with the catalog mismatch keeps validation ahead of
    // identity binding, in that order.
    admin()
        .execute(
            "UPDATE mission.planning_assessments \
         SET contract = jsonb_set(contract, '{basis,revision}', '0') \
         WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
            &[&engagement.0, &campaign.0, &operation.0],
        )
        .unwrap();
    let combined = assessment_row(engagement, campaign, operation);
    for recover in [true, false] {
        assert_eq!(
            planning_assessment::assess(&mut NeverAllocate, &mut store, &input, operation, recover),
            Err(Fail::Store("unsupported_basis"))
        );
        assert_eq!(assessment_row(engagement, campaign, operation), combined);
    }

    // Exact restoration replays the original durable record, not a new one.
    admin()
        .execute(
            "UPDATE mission.planning_assessments SET contract=$4, event_id=$5 \
         WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
            &[
                &engagement.0,
                &campaign.0,
                &operation.0,
                &original_row["contract"],
                &original.event_id.0,
            ],
        )
        .unwrap();
    assert_eq!(
        assessment_row(engagement, campaign, operation),
        original_row
    );
    assert_eq!(
        planning_assessment::assess(&mut NeverAllocate, &mut store, &input, operation, true)
            .unwrap(),
        Some(original)
    );
    assert_eq!(
        count("mission.planning_assessments", engagement, campaign),
        1
    );
}

#[test]
fn missing_owner_and_recovery_absence_are_distinct() {
    let _guard = db();
    let (engagement, campaign) = scope(0xb1a2);
    let (mut allocator, mut store, _trajectory) = ports();
    let operation = OperationId(allocator.allocate().unwrap());
    let input = request(engagement, campaign);
    assert_eq!(
        planning_assessment::assess(&mut allocator, &mut store, &input, operation, true).unwrap(),
        None
    );
    let event = planning_assessment::assess(&mut allocator, &mut store, &input, operation, false)
        .unwrap()
        .expect("missing owner must be durable");
    assert_eq!(event.decision, NonpositiveDecision::UnresolvedMissionBasis);
    assert_eq!(event.basis, None);
    assert_eq!(
        count("mission.planning_assessments", engagement, campaign),
        1
    );
    assert_eq!(
        count("trajectory.registration_history", engagement, campaign),
        0
    );
}
