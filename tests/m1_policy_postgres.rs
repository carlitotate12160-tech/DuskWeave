use duskweave::Fail;
use duskweave::m1_policy::{M1PolicyReader, check_name_policy};
use duskweave::mission::*;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::withdrawal::WithdrawalStore;
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_policy.rs"]
mod support;

#[test]
fn restricted_fresh_reads_are_scoped_durable_and_do_not_write_any_owner_or_history_row() {
    let _guard = db_support::db();
    let (e, c) = db_support::scope(1);
    let event = support::register(&support::registration(e, c, support::now()));
    let before = support::persisted(e, c);
    let mut reader = support::reader();
    for (purpose, reason) in [
        (
            support::discovery("www.example.invalid"),
            "matches_name_policy",
        ),
        (
            support::contact("www.example.invalid", 0x21),
            "matches_name_policy",
        ),
        (
            support::contact("www.example.invalid", 0x22),
            "name_not_permitted",
        ),
        (
            support::discovery("a.excluded.example.invalid"),
            "name_not_permitted",
        ),
    ] {
        let q = support::query(e, c, purpose);
        let result = check_name_policy(&mut reader, &q).unwrap();
        support::assert_reason(
            &result,
            reason,
            if reason == "matches_name_policy" {
                "policy_match"
            } else {
                "policy_no_match"
            },
        );
        let provenance = result.snapshot.unwrap();
        assert_eq!(provenance.registration_operation_id, event.operation_id);
        assert_eq!(provenance.registration_event_id, event.event_id);
        assert_eq!(provenance.authority_revision, 17);
        assert!(result.checked_at.unwrap() > 1700000000);
        assert_eq!(support::persisted(e, c), before);
    }
    let q = support::query(e, c, support::discovery("www.example.invalid"));
    support::assert_reason(
        &check_name_policy(&mut support::reader(), &q).unwrap(),
        "matches_name_policy",
        "policy_match",
    );
    for (scope_e, scope_c) in [
        (e, CampaignId(Uuid::from_u128(99))),
        (EngagementId(Uuid::from_u128(99)), c),
    ] {
        let absent = check_name_policy(
            &mut reader,
            &support::query(scope_e, scope_c, support::discovery("www.example.invalid")),
        )
        .unwrap();
        support::assert_reason(&absent, "mission_missing", "unavailable");
        assert!(absent.checked_at.is_some());
        assert!(absent.snapshot.is_none());
    }
    let (mut allocator, mut store, _) = db_support::ports();
    let request = support::marker(&event).request;
    store
        .withdraw(
            &request,
            OperationId(Uuid::from_u128(0x53)),
            false,
            &mut allocator,
        )
        .unwrap()
        .unwrap();
    let after_withdrawal = support::persisted(e, c);
    for expected in [1, 2] {
        let mut q = q.clone();
        q.expected_mission_revision = expected;
        let result = check_name_policy(&mut support::reader(), &q).unwrap();
        support::assert_reason(&result, "authority_withdrawn", "policy_no_match");
        assert_eq!(result.snapshot.unwrap().effective_mission_revision, 2);
        assert_eq!(support::persisted(e, c), after_withdrawal);
    }
    let (le, lc) = db_support::scope(2);
    let mut legacy = support::registration(le, lc, support::now());
    legacy.as_object_mut().unwrap().remove("m1_permission");
    support::register(&legacy);
    support::assert_reason(
        &check_name_policy(
            &mut support::reader(),
            &support::query(le, lc, support::discovery("www.example.invalid")),
        )
        .unwrap(),
        "m1_policy_absent",
        "policy_no_match",
    );
    for (n, offset) in [(3, -20000), (4, 20000)] {
        let (we, wc) = db_support::scope(n);
        support::register(&support::registration(we, wc, support::now() + offset));
        support::assert_reason(
            &check_name_policy(
                &mut support::reader(),
                &support::query(we, wc, support::discovery("www.example.invalid")),
            )
            .unwrap(),
            "outside_operating_window",
            "policy_no_match",
        );
    }
}

fn assert_corrupt(e: EngagementId, c: CampaignId) {
    let before = support::persisted(e, c);
    let q = support::query(e, c, support::discovery("www.example.invalid"));
    assert_eq!(
        check_name_policy(&mut support::reader(), &q),
        Err(Fail::Store("contract_decode"))
    );
    assert_eq!(support::persisted(e, c), before);
}

#[test]
fn all_registration_versions_bind_catalog_parent_and_outbox_instead_of_legacy_fallback() {
    let _guard = db_support::db();
    let mut admin = support::admin();
    for (n, legacy) in [(10, false), (11, true)] {
        let (e, c) = db_support::scope(n);
        let mut value = support::registration(e, c, support::now());
        if legacy {
            value.as_object_mut().unwrap().remove("m1_permission");
        }
        let event = support::register(&value);
        let original = serde_json::to_value(&event).unwrap();
        let mut corruptions = vec![json!({})];
        for (field, value) in [
            ("version", json!(99)),
            ("owner_revision", json!(0)),
            ("owner_revision", json!(2)),
            ("event_id", json!(Uuid::from_u128(99))),
            ("operation_id", json!(Uuid::from_u128(99))),
            ("producer", json!("not_mission")),
            ("affected_entity", json!(Uuid::from_u128(99))),
            ("correlation_id", json!(Uuid::from_u128(99))),
        ] {
            let mut bad = original.clone();
            bad[field] = value;
            corruptions.push(bad);
        }
        for (field, value) in [
            ("authority_revision", json!(18)),
            (
                "starts_at",
                value["starts_at"].as_i64().map(|t| json!(t - 1)).unwrap(),
            ),
            ("exercise_mode", json!("defender_informed")),
            ("goal_ref", json!(Uuid::from_u128(99))),
        ] {
            let mut bad = original.clone();
            bad["fields"][field] = value;
            corruptions.push(bad);
        }
        if !legacy {
            for permission in [Value::Null, json!({}), {
                let mut bad = original["fields"]["m1_permission"].clone();
                bad["policy_version"] = json!(99);
                bad
            }] {
                let mut bad = original.clone();
                bad["fields"]["m1_permission"] = permission;
                corruptions.push(bad);
            }
        }
        for bad in corruptions {
            admin.execute("UPDATE mission.registration_outbox SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2",
                &[&e.0, &c.0, &bad]).unwrap();
            assert_corrupt(e, c);
        }
        admin.execute("UPDATE mission.registration_outbox SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0, &original]).unwrap();
        admin.execute("UPDATE mission.registration_outbox SET event_id=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0, &Uuid::from_u128(98)]).unwrap();
        assert_corrupt(e, c);
        admin.execute("UPDATE mission.registration_outbox SET event_id=$3, operation_id=$4 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0, &event.event_id.0, &Uuid::from_u128(98)]).unwrap();
        assert_corrupt(e, c);
        admin.execute("UPDATE mission.registration_outbox SET operation_id=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0, &event.operation_id.0]).unwrap();
        admin.execute("UPDATE mission.missions SET authority_revision=19 WHERE engagement_id=$1 AND campaign_id=$2", &[&e.0, &c.0]).unwrap();
        assert_corrupt(e, c);
    }
}

#[test]
fn withdrawal_codec_and_original_registration_lineage_reject_corrupt_and_orphan_markers() {
    let _guard = db_support::db();
    let (e, c) = db_support::scope(20);
    let event = support::register(&support::registration(e, c, support::now()));
    let marker = support::marker(&event);
    let mut admin = support::admin();
    support::insert_marker(&mut admin, &marker);
    let original = serde_json::to_value(&marker).unwrap();
    for (field, value) in [
        ("version", json!(99)),
        ("owner_revision", json!(1)),
        ("registration_operation_id", json!(Uuid::from_u128(99))),
        ("event_id", json!(Uuid::from_u128(99))),
        ("producer", json!("other")),
    ] {
        let mut bad = original.clone();
        bad[field] = value;
        admin.execute("UPDATE mission.withdrawals SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2", &[&e.0, &c.0, &bad]).unwrap();
        assert_corrupt(e, c);
    }
    let mut bad = original.clone();
    bad["request"]["engagement_id"] = json!(Uuid::from_u128(99));
    admin
        .execute(
            "UPDATE mission.withdrawals SET contract=$3 WHERE engagement_id=$1 AND campaign_id=$2",
            &[&e.0, &c.0, &bad],
        )
        .unwrap();
    assert_corrupt(e, c);
    let mut wrong_lineage = marker.clone();
    wrong_lineage.registration_operation_id = OperationId(Uuid::from_u128(99));
    admin.execute("UPDATE mission.withdrawals SET registration_operation_id=$3,contract=$4 WHERE engagement_id=$1 AND campaign_id=$2",
        &[&e.0, &c.0, &wrong_lineage.registration_operation_id.0, &serde_json::to_value(&wrong_lineage).unwrap()]).unwrap();
    assert_corrupt(e, c);
    let (oe, oc) = db_support::scope(21);
    let unregistered = support::event(&support::registration(oe, oc, support::now()));
    support::insert_marker(&mut admin, &support::marker(&unregistered));
    assert_corrupt(oe, oc);
    let (ae, ac) = db_support::scope(22);
    support::assert_reason(
        &check_name_policy(
            &mut support::reader(),
            &support::query(ae, ac, support::discovery("www.example.invalid")),
        )
        .unwrap(),
        "mission_missing",
        "unavailable",
    );
}

#[test]
fn blocked_withdrawal_read_retains_the_first_select_snapshot_and_next_read_sees_commit() {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    let _guard = db_support::db();
    let (e, c) = db_support::scope(30);
    let event = support::register(&support::registration(e, c, support::now()));
    let before = support::persisted(e, c);
    let marker = support::marker(&event);
    let mut lock_owner = support::admin();
    let mut tx = lock_owner.transaction().unwrap();
    tx.batch_execute("LOCK TABLE mission.withdrawals IN ACCESS EXCLUSIVE MODE")
        .unwrap();
    let mut observer = support::admin();
    let mut reader_client = db_support::runtime_client();
    let pid: i32 = reader_client
        .query_one("SELECT pg_backend_pid()", &[])
        .unwrap()
        .get(0);
    let (start_tx, start_rx) = mpsc::sync_channel(0);
    let (result_tx, result_rx) = mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut store = PgMissionStore::new(reader_client);
        start_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        result_tx.send(store.read_policy_snapshot(e, c)).unwrap();
    });
    start_tx.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let waiting: bool = observer.query_one("SELECT EXISTS (SELECT 1 FROM pg_locks WHERE pid=$1 \
            AND relation='mission.withdrawals'::regclass AND mode='AccessShareLock' AND NOT granted)", &[&pid]).unwrap().get(0);
        if waiting {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "exact owner reader did not reach withdrawal relation"
        );
    }
    assert!(result_rx.try_recv().is_err());
    support::insert_marker(&mut tx, &marker);
    tx.commit().unwrap();
    let snapshot = result_rx
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .unwrap();
    reader.join().unwrap();
    struct Captured(duskweave::m1_policy::M1PolicySnapshot);
    impl M1PolicyReader for Captured {
        fn read_policy_snapshot(
            &mut self,
            _: EngagementId,
            _: CampaignId,
        ) -> duskweave::Res<duskweave::m1_policy::M1PolicySnapshot> {
            Ok(self.0.clone())
        }
    }
    let q = support::query(e, c, support::discovery("www.example.invalid"));
    let overlapped = check_name_policy(&mut Captured(snapshot), &q).unwrap();
    support::assert_reason(&overlapped, "matches_name_policy", "policy_match");
    assert_eq!(overlapped.snapshot.unwrap().effective_mission_revision, 1);
    let after = support::persisted(e, c);
    for index in [0, 1, 2, 4, 5, 6] {
        assert_eq!(before[index], after[index]);
    }
    assert_eq!(before[3], json!([]));
    assert_eq!(after[3].as_array().unwrap().len(), 1);
    support::assert_reason(
        &check_name_policy(&mut support::reader(), &q).unwrap(),
        "authority_withdrawn",
        "policy_no_match",
    );
    assert_eq!(support::persisted(e, c), after);
}
