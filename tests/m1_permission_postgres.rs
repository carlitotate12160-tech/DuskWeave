use duskweave::Fail;
use duskweave::input::parse_register;
use duskweave::mission::*;
use duskweave::registration::{
    self, MissionStore, OperationAllocator, ReconcileOutcome, TrajectoryPort,
};
use duskweave::trajectory::{Delivered, HistoryStatus};
use duskweave::withdrawal::{
    WithdrawalHistoryPort, WithdrawalReason, WithdrawalRequest, WithdrawalStore,
};
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_permission.rs"]
mod policy;
use db_support::*;

fn admitted(n: u128) -> (registration::Receipt, MissionRegistered) {
    let (e, c) = scope(n);
    let (mut a, mut s, mut t) = ports();
    let op = registration::prepare_operation(&mut a).unwrap();
    let receipt = registration::register(&mut a, &mut s, &mut t, op, &policy::input(e, c)).unwrap();
    let event = s.outbox_event(e, c, op).unwrap().unwrap();
    (receipt, event)
}

fn stored_contract(table: &str, e: EngagementId, c: CampaignId) -> Value {
    runtime_client()
        .query_one(
            &format!("SELECT contract FROM {table} WHERE engagement_id=$1 AND campaign_id=$2"),
            &[&e.0, &c.0],
        )
        .unwrap()
        .get(0)
}

fn corrupt(receipt: &registration::Receipt, contract: &Value) {
    let mut cfg = dsn("DW_TEST_ADMIN_DATABASE_URL");
    cfg.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    assert_eq!(cfg.connect(postgres::NoTls).unwrap().execute(
        "UPDATE mission.registration_outbox SET contract=$4 WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3",
        &[&receipt.engagement_id.0, &receipt.campaign_id.0, &receipt.operation_id.0, contract],
    ).unwrap(), 1);
}

#[test]
fn initial_attachment_is_durable_immutable_and_scoped() {
    let _g = db();
    let (receipt, event) = admitted(0x7101);
    let (e, c, op) = (
        receipt.engagement_id,
        receipt.campaign_id,
        receipt.operation_id,
    );
    assert_eq!(receipt.history, HistoryStatus::Completed);
    let contract = serde_json::to_value(&event).unwrap();
    assert_eq!(contract["fields"]["m1_permission"], policy::permission());
    assert_eq!(
        stored_contract("mission.registration_outbox", e, c),
        contract
    );
    assert_eq!(
        stored_contract("trajectory.registration_history", e, c),
        contract
    );
    let (mut a, mut s, mut t) = ports();
    let inspected = registration::inspect(&mut s, &mut t, e, c)
        .unwrap()
        .unwrap();
    let summary = inspected.m1_permission.unwrap();
    assert_eq!(summary.campaign_limits.dns_questions, 10);
    assert_eq!(summary.vantage_ref, Uuid::from_u128(0x31));
    assert_eq!(summary.authority_ref, AuthorityRef(Uuid::from_u128(0x12)));
    assert_eq!(summary.contact_rules, 1);
    assert_eq!(summary.discovery_rules, 1);
    assert_eq!(
        registration::register(&mut a, &mut s, &mut t, op, &policy::input(e, c)).unwrap(),
        receipt
    );
    assert_eq!(
        registration::reconcile(&mut s, &mut t, e, c, op),
        Ok(ReconcileOutcome::Committed)
    );
    for pointer in [
        "/m1_permission/campaign_limits/episodes",
        "/m1_permission/contact_rules/0/priority",
    ] {
        let mut changed = policy::registration(e, c);
        *changed.pointer_mut(pointer).unwrap() = json!(4);
        let input = parse_register(&serde_json::to_vec(&changed).unwrap()).unwrap();
        assert_eq!(
            registration::register(&mut a, &mut s, &mut t, op, &input),
            Err(Fail::Conflict("integrity_conflict"))
        );
    }
    let mut changed = policy::registration(e, c);
    changed["m1_permission"]["contact_rules"][0]["rule"] = json!({"exact": "changed.example.net"});
    let input = parse_register(&serde_json::to_vec(&changed).unwrap()).unwrap();
    assert_eq!(
        registration::register(&mut a, &mut s, &mut t, op, &input),
        Err(Fail::Conflict("integrity_conflict"))
    );
    let other = registration::prepare_operation(&mut a).unwrap();
    assert_eq!(
        registration::register(&mut a, &mut s, &mut t, other, &input),
        Err(Fail::Conflict("already_registered"))
    );
    assert_eq!(
        stored_contract("mission.registration_outbox", e, c),
        contract
    );
    assert_eq!(
        stored_contract("trajectory.registration_history", e, c),
        contract
    );
    for table in [
        "mission.missions",
        "mission.registration_outbox",
        "trajectory.registration_history",
    ] {
        assert_eq!(count(table, e, c), 1);
    }
    let (e2, c2) = scope(0x7199);
    assert!(
        registration::inspect(&mut s, &mut t, e2, c)
            .unwrap()
            .is_none()
    );
    assert!(
        registration::inspect(&mut s, &mut t, e, c2)
            .unwrap()
            .is_none()
    );
    assert!(s.outbox_event(e, c2, op).unwrap().is_none());
}

#[test]
fn conflicting_valid_redelivery_cannot_replace_or_clear_history() {
    let _g = db();
    let (receipt, event) = admitted(0x7201);
    let (e, c) = (receipt.engagement_id, receipt.campaign_id);
    let original = stored_contract("trajectory.registration_history", e, c);
    let mut changed = original.clone();
    changed["fields"]["m1_permission"]["campaign_limits"]["episodes"] = json!(9);
    let changed: MissionRegistered = serde_json::from_value(changed).unwrap();
    let (_, mut s, mut t) = ports();
    assert_eq!(t.deliver(&changed).unwrap(), Delivered::Anomaly);
    assert_eq!(t.deliver(&event).unwrap(), Delivered::Anomaly);
    assert_eq!(
        t.status(e, c, receipt.event_id).unwrap(),
        HistoryStatus::Anomaly
    );
    let accepted: Value = runtime_client().query_one(
        "SELECT contract FROM trajectory.registration_history WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
        &[&e.0, &c.0],
    ).unwrap().get(0);
    assert_eq!(accepted, original);
    assert_eq!(
        count_where(
            "trajectory.registration_history",
            "AND status='accepted'",
            e,
            c
        ),
        1
    );
    assert_eq!(
        count_where(
            "trajectory.registration_history",
            "AND status='anomaly' AND contract IS NULL",
            e,
            c
        ),
        1
    );
    let view = registration::inspect(&mut s, &mut t, e, c)
        .unwrap()
        .unwrap();
    assert_eq!(view.history, HistoryStatus::Anomaly);
    assert!(view.m1_permission.is_some());
    assert_eq!(
        registration::reconcile(&mut s, &mut t, e, c, receipt.operation_id),
        Ok(ReconcileOutcome::Conflicted)
    );
}

#[test]
fn pending_and_withdrawn_history_preserves_original_configuration() {
    let _g = db();
    let (e, c) = scope(0x7301);
    let input = policy::input(e, c);
    let (mut a, mut s, mut t) = ports();
    let op = registration::prepare_operation(&mut a).unwrap();
    let ev_id = EventId(a.allocate().unwrap());
    let (mission, event) = Mission::register(&input, op, ev_id, 1700000001).unwrap();
    s.commit_registration(&mission, &event).unwrap();
    let view = registration::inspect(&mut s, &mut t, e, c)
        .unwrap()
        .unwrap();
    assert_eq!(view.history, HistoryStatus::Pending);
    assert!(view.m1_permission.is_some());
    assert_eq!(count("trajectory.registration_history", e, c), 0);
    assert_eq!(
        registration::reconcile(&mut s, &mut t, e, c, op),
        Ok(ReconcileOutcome::Committed)
    );
    let request = WithdrawalRequest {
        engagement_id: e,
        campaign_id: c,
        operator_ref: OperatorRef(Uuid::from_u128(0x11)),
        expected_mission_revision: 1,
        reason: WithdrawalReason::OperatorRequested,
    };
    let withdrawal_op = registration::prepare_operation(&mut a).unwrap();
    let withdrawal = s
        .withdraw(&request, withdrawal_op, false, &mut a)
        .unwrap()
        .unwrap();
    assert_eq!(
        WithdrawalHistoryPort::publish(&mut t, &withdrawal).unwrap(),
        Delivered::Completed
    );
    let (_, mut s, mut t) = ports();
    let withdrawn = registration::inspect(&mut s, &mut t, e, c)
        .unwrap()
        .unwrap();
    assert_eq!(withdrawn.mission.revision, 2);
    assert_eq!(withdrawn.event_id, Some(event.event_id));
    assert_eq!(withdrawn.m1_permission, view.m1_permission);
    assert_eq!(s.outbox_event(e, c, op).unwrap().unwrap(), event);
    assert_eq!(
        registration::reconcile(&mut s, &mut t, e, c, op),
        Ok(ReconcileOutcome::Committed)
    );
}

#[test]
fn catalog_parent_and_payload_corruption_fails_duplicate_and_inspection() {
    let _g = db();
    let cases = [
        ("/fields/goal_ref", json!(Uuid::from_u128(0x99))),
        ("/fields/operator_ref", json!(Uuid::from_u128(0x99))),
        ("/fields/authority_ref", json!(Uuid::from_u128(0x99))),
        ("/fields/authority_revision", json!(2)),
        ("/fields/included_assets", json!([Uuid::from_u128(0x21)])),
        ("/fields/excluded_assets", json!([])),
        ("/fields/exercise_mode", json!("defender_informed")),
        ("/fields/starts_at", json!(1699999999)),
        ("/fields/ends_at", json!(1700086401)),
        ("/event_id", json!(Uuid::from_u128(0x99))),
        ("/engagement_id", json!(Uuid::from_u128(0x99))),
        ("/owner_revision", json!(2)),
        ("/producer", json!("other")),
        ("/kind", json!("other")),
        ("/version", json!(1)),
        ("/version", json!(3)),
        ("/fields/m1_permission", Value::Null),
        ("/fields/m1_permission/policy_version", json!(2)),
        ("/fields/m1_permission/campaign_limits/episodes", json!(0)),
    ];
    for (i, (pointer, value)) in cases.into_iter().enumerate() {
        let (receipt, event) = admitted(0x7400 + i as u128);
        let (e, c, op) = (
            receipt.engagement_id,
            receipt.campaign_id,
            receipt.operation_id,
        );
        let mut altered = serde_json::to_value(event).unwrap();
        *altered.pointer_mut(pointer).unwrap() = value;
        corrupt(&receipt, &altered);
        let (mut a, mut s, mut t) = ports();
        assert_eq!(
            registration::inspect(&mut s, &mut t, e, c),
            Err(Fail::Store("contract_decode")),
            "{pointer}"
        );
        assert_eq!(
            registration::register(&mut a, &mut s, &mut t, op, &policy::input(e, c)),
            Err(Fail::Store("contract_decode")),
            "{pointer}"
        );
        assert_eq!(count("mission.missions", e, c), 1);
        assert_eq!(count("trajectory.registration_history", e, c), 1);
    }
    let (receipt, event) = admitted(0x7480);
    let mut altered = serde_json::to_value(event).unwrap();
    let campaign = json!(Uuid::from_u128(0x99));
    altered["campaign_id"] = campaign.clone();
    altered["affected_entity"] = campaign;
    corrupt(&receipt, &altered);
    let (_, mut s, mut t) = ports();
    assert_eq!(
        registration::inspect(&mut s, &mut t, receipt.engagement_id, receipt.campaign_id),
        Err(Fail::Store("contract_decode"))
    );
}

#[test]
fn corrupted_catalog_identity_is_not_a_valid_attachment_lookup() {
    let _g = db();
    let (receipt, event) = admitted(0x7501);
    let mut cfg = dsn("DW_TEST_ADMIN_DATABASE_URL");
    cfg.dbname(dsn("DW_TEST_DATABASE_URL").get_dbname().unwrap());
    let mut admin = cfg.connect(postgres::NoTls).unwrap();
    admin.execute("UPDATE mission.registration_outbox SET event_id=$3 WHERE engagement_id=$1 AND campaign_id=$2",
        &[&receipt.engagement_id.0, &receipt.campaign_id.0, &Uuid::from_u128(0x99)]).unwrap();
    let (mut a, mut s, mut t) = ports();
    assert_eq!(
        registration::inspect(&mut s, &mut t, receipt.engagement_id, receipt.campaign_id),
        Err(Fail::Store("contract_decode"))
    );
    assert_eq!(
        registration::register(
            &mut a,
            &mut s,
            &mut t,
            receipt.operation_id,
            &policy::input(receipt.engagement_id, receipt.campaign_id)
        ),
        Err(Fail::Store("contract_decode"))
    );
    assert_eq!(t.deliver(&event).unwrap(), Delivered::Duplicate);
}

#[test]
fn concurrent_m1_registration_has_one_committed_policy() {
    let _g = db();
    let (e, c) = scope(0x7601);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|index| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let (mut a, mut s, mut t) = ports();
                let op = registration::prepare_operation(&mut a).unwrap();
                let mut raw = policy::registration(e, c);
                raw["m1_permission"]["campaign_limits"]["episodes"] = json!(index + 2);
                let input = parse_register(&serde_json::to_vec(&raw).unwrap()).unwrap();
                barrier.wait();
                retry(|| registration::register(&mut a, &mut s, &mut t, op, &input))
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(Fail::Conflict(_))))
            .count(),
        1
    );
    for table in [
        "mission.missions",
        "mission.registration_outbox",
        "trajectory.registration_history",
    ] {
        assert_eq!(count(table, e, c), 1);
    }
    assert_eq!(
        stored_contract("mission.registration_outbox", e, c),
        stored_contract("trajectory.registration_history", e, c)
    );
}

#[test]
fn attachment_inspection_rejects_unbound_public_port_views_and_exposes_missing_history() {
    let (e, c) = scope(0x7b01);
    let (_, event) = Mission::register(
        &policy::input(e, c),
        OperationId(Uuid::from_u128(1)),
        EventId(Uuid::from_u128(2)),
        3,
    )
    .unwrap();
    let view = registration::MissionView {
        operation_id: event.operation_id,
        revision: 2,
        exercise_mode: event.fields.exercise_mode(),
        starts_at: event.fields.starts_at(),
        ends_at: event.fields.ends_at(),
    };
    let mut store = policy::InspectPorts {
        event: Some(event.clone()),
        view: view.clone(),
    };
    let mut history = policy::InspectHistory;
    assert!(
        registration::inspect(&mut store, &mut history, e, c)
            .unwrap()
            .unwrap()
            .m1_permission
            .is_some()
    );
    for index in 0..4 {
        store.view = view.clone();
        match index {
            0 => store.view.operation_id = OperationId(Uuid::from_u128(99)),
            1 => store.view.exercise_mode = ExerciseMode::DefenderInformed,
            2 => store.view.starts_at -= 1,
            _ => store.view.ends_at += 1,
        }
        assert_eq!(
            registration::inspect(&mut store, &mut history, e, c),
            Err(Fail::Store("contract_decode"))
        );
    }
    store.view = view;
    assert_eq!(
        registration::inspect(
            &mut store,
            &mut history,
            EngagementId(Uuid::from_u128(99)),
            c
        ),
        Err(Fail::Store("contract_decode"))
    );
    assert_eq!(
        registration::inspect(&mut store, &mut history, e, CampaignId(Uuid::from_u128(99))),
        Err(Fail::Store("contract_decode"))
    );
    store.event = None;
    let missing = registration::inspect(&mut store, &mut history, e, c)
        .unwrap()
        .unwrap();
    assert_eq!(missing.history, HistoryStatus::Pending);
    assert_eq!(missing.event_id, None);
    assert_eq!(missing.m1_permission, None);
}
