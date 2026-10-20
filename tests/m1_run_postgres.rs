//! Real binary -> fixed child -> restricted owners -> feedback, with no external endpoint.
#[path = "support/registration_db.rs"]
mod db_support;
use terrain::terrain_support::native;
#[path = "support/m1_terrain_context.rs"]
mod terrain;
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/m1_run.rs"]
mod run_support;
use run_support::*;
#[test]
fn eight_seed_production_run_fits_both_full_calls_and_rebinds_actual_selected_seed() {
    let _guard = native::db();
    refresh();
    let q = registered_with(0xa711, |raw| {
        let rule = raw["m1_permission"]["contact_rules"][0].clone();
        let assets: Vec<_> = (0..8)
            .map(|n| Uuid::from_u128(if n == 0 { 0x21 } else { 0x100 + n }))
            .collect();
        raw["included_assets"] = json!(assets);
        raw["m1_permission"]["contact_rules"] = json!(
            assets
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let mut r = rule.clone();
                    r["asset_ref"] = json!(id);
                    r["priority"] = json!(i + 1);
                    if i > 0 {
                        r["rule"] = json!({"exact":format!("seed{i}.example.invalid")});
                    }
                    r
                })
                .collect::<Vec<_>>()
        );
    });
    let path = path(&q);
    let (ok, v) = cli("m1-run", "--input", path.to_str().unwrap());
    std::fs::remove_file(path).unwrap();
    assert!(ok, "{v}");
    assert_eq!(v["decision_count"], 2);
    assert_eq!(v["effect_count"], 1);
    let r = id(&q);
    for row in native::admin().query("SELECT p::jsonb FROM execution.inference_calls WHERE d IN (SELECT decision_one FROM execution.autonomous_runs WHERE run_id=$1 UNION ALL SELECT decision_two FROM execution.autonomous_runs WHERE run_id=$1)",&[&r]).unwrap() {
        let pack:Value=row.get(0);let p=serde_json::to_vec(&pack).unwrap();assert!(p.len()<=8192);
        assert_eq!(pack["candidates"].as_array().unwrap().len(),8);
        for repair in [None,Some(&["unexpected_view_request";8][..])] {
            let call=duskweave::m1_reasoning::request::bind(&p,repair).unwrap();
            assert!(call.body.len()<=16384);assert!(call.configuration.len()<=16384);
            assert_eq!(serde_json::from_slice::<Value>(&call.configuration).unwrap()["requested_settings"]["model"],"deterministic-autonomous-v1");
        }
        if pack["predecessor"]["availability"]=="committed" {
            let label=pack["predecessor"]["selected_seed_reference"].as_str().unwrap();
            assert!(pack["references"].as_array().unwrap().iter().any(|r|r["id"]==label && r["entity"]==q["seed_names"][0]["asset_ref"]));
            assert!(pack["predecessor"].get("selected_seed_entity").is_none());
        }
    }
}
#[test]
fn production_run_links_two_decisions_and_repeated_start_and_recovery_have_zero_replay() {
    let _guard = native::db();
    refresh();
    let q = registered(0xa710);
    let path = std::env::temp_dir().join(format!("dw-run-{}.json", q["run_id"].as_str().unwrap()));
    std::fs::write(&path, serde_json::to_vec(&q).unwrap()).unwrap();
    {
        let mut b = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
        let mut tx = b.transaction().unwrap();
        let check = tx.query_one("SELECT execution.run_start($1,$2)", &[&q, &"a".repeat(64)]);
        assert!(
            check.is_ok(),
            "run_start SQL: {}",
            check
                .err()
                .and_then(|e| e.as_db_error().map(|e| e.message().to_owned()))
                .unwrap_or_default()
        );
        tx.rollback().unwrap();
    }
    let (ok, receipt) = cli("m1-run", "--input", path.to_str().unwrap());
    println!("{receipt}");
    assert!(ok, "{receipt}");
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["decision_count"], 2);
    assert_eq!(receipt["effect_count"], 1);
    assert_eq!(receipt["decision_history_count"], 2);
    assert_eq!(receipt["terrain_history_count"], 1);
    assert_eq!(receipt["execution_mode"], "controlled_fixture");
    assert_eq!(receipt["dispatch_granted"], false);
    let e = Uuid::parse_str(q["context"]["mission"]["engagement_id"].as_str().unwrap()).unwrap();
    let c = Uuid::parse_str(q["context"]["mission"]["campaign_id"].as_str().unwrap()).unwrap();
    let mut a = native::admin();
    let rows = a
        .query(
            "SELECT p::jsonb FROM execution.inference_calls WHERE e=$1 AND c=$2 ORDER BY d",
            &[&e, &c],
        )
        .unwrap();
    assert_eq!(rows.len(), 2);
    let packs: Vec<Value> = rows.iter().map(|r| r.get(0)).collect();
    let first = packs
        .iter()
        .find(|p| p["predecessor"]["availability"] == "absent")
        .unwrap();
    let first_terrain = first["owner_views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["owner"] == "cyber_terrain")
        .unwrap();
    assert_eq!(first_terrain["availability"], "available");
    assert_eq!(first_terrain["shown_claims"], 0);
    assert!(
        first_terrain.get("source").is_none(),
        "empty first Terrain view must omit source: {first_terrain}"
    );
    let second = packs
        .iter()
        .find(|p| p["predecessor"]["availability"] == "committed")
        .unwrap();
    let second_terrain = second["owner_views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["owner"] == "cyber_terrain")
        .unwrap();
    assert_eq!(second_terrain["availability"], "available");
    assert_eq!(second_terrain["shown_claims"], 1);
    assert_eq!(second_terrain["source"], "fixture_dns_acquisition");
    assert_eq!(second["predecessor"]["result_ref"], receipt["result"]);
    assert!(
        second["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["source"] == "fixture_dns_acquisition")
    );
    let proposal: Value = a
        .query_one(
            "SELECT payload->'proposal' FROM cognition.reasoning_decisions WHERE d=$1",
            &[&Uuid::parse_str(receipt["decision_two"].as_str().unwrap()).unwrap()],
        )
        .unwrap()
        .get(0);
    assert!(
        proposal["references"]
            .as_array()
            .unwrap()
            .contains(&json!("previous_decision"))
    );
    assert!(
        proposal["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r.as_str().unwrap().starts_with("terrain_"))
    );
    for command in ["m1-run-inspect", "m1-run-recover", "m1-run-recover"] {
        let (ok, v) = cli(command, "--run", q["run_id"].as_str().unwrap());
        assert!(ok, "{v}");
        for k in [
            "run_id",
            "decision_one",
            "decision_two",
            "attempt",
            "result",
            "effect_count",
            "decision_count",
        ] {
            assert_eq!(v[k], receipt[k]);
        }
    }
    let (ok, v) = cli("m1-run", "--input", path.to_str().unwrap());
    assert!(ok, "{v}");
    assert_eq!(v["effect_count"], 1);
    let mut conflicting = q.clone();
    conflicting["seed_names"][0]["name"] = json!("conflict.example.invalid");
    std::fs::write(&path, serde_json::to_vec(&conflicting).unwrap()).unwrap();
    assert!(!cli("m1-run", "--input", path.to_str().unwrap()).0);
    std::fs::remove_file(path).unwrap();
}
