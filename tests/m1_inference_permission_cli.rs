use duskweave::input::parse_register;
use duskweave::mission::*;
use duskweave::registration::{self, MissionStore, TrajectoryPort};
use duskweave::trajectory::{Delivered, HistoryStatus};
use serde_json::{Value, json};
use uuid::Uuid;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/m1_permission.rs"]
mod ct;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/database_wait_db.rs"]
mod wait_db;

fn inspect(e: EngagementId, c: CampaignId) -> std::process::Output {
    consent::cli(&[
        "inspect",
        "--engagement",
        &e.to_string(),
        "--campaign",
        &c.to_string(),
    ])
}
fn historical(out: &std::process::Output) -> Value {
    assert!(out.status.success(), "inspection failed");
    consent::text(out)
        .lines()
        .filter_map(|s| serde_json::from_str::<Value>(s).ok())
        .find(|v| v["kind"] == "m1_inference_historical_configuration_v1")
        .expect("consent receipt")
}
fn flags(v: &Value) {
    for flag in [
        "current_permission",
        "dispatch_granted",
        "provider_qualified",
        "acquisition_qualified",
    ] {
        assert_eq!(v[flag], false, "{flag}");
    }
}

#[test]
fn real_cli_durable_outbox_history_fresh_inspection_duplicate_and_withdrawal() {
    let _g = db_support::db();
    for both in [false, true] {
        let (e, c) = db_support::scope(0xb100 + u128::from(both));
        let mut raw = consent::registration(e, c);
        if both {
            raw["m1_permission"] = ct::permission();
        }
        let op = consent::id().to_string();
        let file = wait_db::InputFile::new(&raw, &op);
        let args = ["register", "--operation", &op, "--input", file.path()];
        let first = consent::cli(&args);
        assert!(first.status.success(), "register failed");
        assert!(consent::text(&first).contains("history=completed"));
        assert_eq!(consent::text(&consent::cli(&args)), consent::text(&first));
        let out = inspect(e, c);
        let view = historical(&out);
        assert_eq!(view["attachment"], raw["m1_inference_permission"]);
        assert_eq!(view["history"], "completed");
        flags(&view);
        assert_eq!(
            consent::text(&out).contains("m1 historical_configuration"),
            both
        );
        let (_, mut s, mut t) = db_support::ports();
        let original = s
            .outbox_event(e, c, OperationId::parse(&op).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(original.version, 3);
        let durable:Value=db_support::runtime_client().query_one(
            "SELECT contract FROM trajectory.registration_history WHERE engagement_id=$1 AND campaign_id=$2 AND status='accepted'",
            &[&e.0,&c.0]).unwrap().get(0);
        assert_eq!(durable, serde_json::to_value(&original).unwrap());
        let mut changed = raw.clone();
        changed["m1_inference_permission"]["campaign_limits"]["calls"] = json!(11);
        let changed_file = wait_db::InputFile::new(&changed, "changed-consent");
        let conflict = consent::cli(&[
            "register",
            "--operation",
            &op,
            "--input",
            changed_file.path(),
        ]);
        assert!(!conflict.status.success());
        assert!(consent::text(&conflict).contains("integrity_conflict"));
        let amendment = consent::cli(&[
            "register",
            "--operation",
            &consent::id().to_string(),
            "--input",
            file.path(),
        ]);
        assert!(!amendment.status.success());
        assert!(consent::text(&amendment).contains("already_registered"));
        let reconciled = consent::cli(&[
            "reconcile",
            "--engagement",
            &e.to_string(),
            "--campaign",
            &c.to_string(),
            "--operation",
            &op,
        ]);
        assert!(reconciled.status.success());
        let withdrawal = json!({"engagement_id":e,"campaign_id":c,"operator_ref":Uuid::from_u128(0x11),
            "expected_mission_revision":1,"reason":"operator_requested"});
        let withdrawal_file = wait_db::InputFile::new(&withdrawal, "withdraw-consent");
        let withdrawn = consent::cli(&[
            "withdraw",
            "--operation",
            &consent::id().to_string(),
            "--input",
            withdrawal_file.path(),
            "--recover",
            "false",
        ]);
        assert!(withdrawn.status.success(), "withdraw failed");
        let after = inspect(e, c);
        assert!(consent::text(&after).contains("mission.revision=2"));
        assert_eq!(historical(&after), view);
        assert_eq!(
            s.outbox_event(e, c, original.operation_id)
                .unwrap()
                .unwrap(),
            original
        );
        assert_eq!(
            t.registration_status(&original).unwrap(),
            HistoryStatus::Completed
        );
        consent::assert_rows(e, c, 1);
    }
}

#[test]
fn pending_publication_recovers_without_rewriting_consent_and_anomaly_dominates() {
    let _g = db_support::db();
    let (e, c) = db_support::scope(0xb200);
    let raw = consent::registration(e, c);
    let input = parse_register(&serde_json::to_vec(&raw).unwrap()).unwrap();
    let op = OperationId(consent::id());
    let (mission, event) = Mission::register(&input, op, EventId(consent::id()), 5).unwrap();
    let (_, mut s, mut t) = db_support::ports();
    s.commit_registration(&mission, &event).unwrap();
    let pending = historical(&inspect(e, c));
    assert_eq!(pending["history"], "pending");
    flags(&pending);
    consent::assert_rows(e, c, 0);
    let recovered = consent::cli(&[
        "reconcile",
        "--engagement",
        &e.to_string(),
        "--campaign",
        &c.to_string(),
        "--operation",
        &op.to_string(),
    ]);
    assert!(recovered.status.success());
    assert_eq!(historical(&inspect(e, c))["history"], "completed");
    let mut variant = serde_json::to_value(&event).unwrap();
    variant["fields"]["m1_inference_permission"]["campaign_limits"]["calls"] = json!(11);
    let variant = serde_json::from_value(variant).unwrap();
    assert_eq!(t.deliver(&variant).unwrap(), Delivered::Anomaly);
    assert_eq!(t.deliver(&event).unwrap(), Delivered::Anomaly);
    let anomalous = historical(&inspect(e, c));
    assert_eq!(anomalous["history"], "anomaly");
    assert_eq!(anomalous["attachment"], raw["m1_inference_permission"]);
    flags(&anomalous);
    assert_eq!(
        registration::inspect(&mut s, &mut t, e, c)
            .unwrap()
            .unwrap()
            .history,
        HistoryStatus::Anomaly
    );
}

#[test]
fn legacy_registration_cannot_be_amended_and_rejected_sentinel_never_reaches_sql_or_cli() {
    let _g = db_support::db();
    let (e, c) = db_support::scope(0xb300);
    let (mut a, mut s, mut t) = db_support::ports();
    let receipt = db_support::accepted(&mut a, &mut s, &mut t, e, c);
    let raw = consent::registration(e, c);
    for op in [receipt.operation_id, OperationId(consent::id())] {
        let file = wait_db::InputFile::new(&raw, &op.to_string());
        assert!(
            !consent::cli(&[
                "register",
                "--operation",
                &op.to_string(),
                "--input",
                file.path()
            ])
            .status
            .success()
        );
    }
    let old = inspect(e, c);
    assert!(!consent::text(&old).contains("m1_inference_historical"));
    consent::assert_rows(e, c, 1);
    let (e, c) = db_support::scope(0xb301);
    let mut bad = consent::registration(e, c);
    bad["m1_inference_permission"]["secret"] = json!("SYNTHETIC_SECRET_SENTINEL");
    let file = wait_db::InputFile::new(&bad, "sensitive-reject");
    let out = consent::cli(&[
        "register",
        "--operation",
        &consent::id().to_string(),
        "--input",
        file.path(),
    ]);
    assert!(!out.status.success());
    assert!(!consent::text(&out).contains("SYNTHETIC_SECRET_SENTINEL"));
    assert!(out.stderr.is_empty());
    for table in [
        "mission.missions",
        "mission.registration_outbox",
        "trajectory.registration_history",
    ] {
        assert_eq!(db_support::count(table, e, c), 0);
    }
}
