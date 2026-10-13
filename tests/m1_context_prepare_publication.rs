use duskweave::Fail;
use duskweave::m1_context::prepare;
use duskweave::postgres_mission::PgContextReader;
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/m1_inference_budget.rs"]
mod budget;
#[path = "support/m1_inference_permission.rs"]
mod consent;
#[path = "support/m1_context_prepare.rs"]
mod context_support;
#[path = "support/registration_db.rs"]
mod db_support;
#[path = "support/m1_context_publication_fault.rs"]
mod publication_fault;
#[path = "support/m1_session.rs"]
mod session_support;
#[path = "support/database_wait_db.rs"]
mod wait_db;

use publication_fault::PublicationFault;

fn publication_case(
    seed: u128,
    expected: Fail,
    cli: Option<&str>,
    mutate: impl FnOnce(&mut PublicationFault),
) {
    let _g = db_support::db();
    let (req, source) = budget::fixture(seed);
    let q = context_support::context_request(&req);
    let (e, c) = (req.engagement_id.0, req.campaign_id.0);
    let original = context_support::scoped_state(e, c);
    context_support::assert_mission_frontier(&context_support::prepare_pack(&q), &source);
    assert_eq!(original, context_support::scoped_state(e, c));
    PublicationFault::capture(e, c).checked(|fault| {
        mutate(fault);
        let corrupted = context_support::scoped_state(e, c);
        let mut reader = PgContextReader::new(session_support::broker_client());
        assert_eq!(prepare(&mut reader, &q, budget::op()).err(), Some(expected));
        assert_eq!(corrupted, context_support::scoped_state(e, c));
        fault.assert_foreign_unchanged();
        if let Some(outcome) = cli {
            let out = context_support::run_request(&q, "publication_matrix");
            context_support::assert_failed_receipt(&out, outcome);
            for stream in [&out.stdout, &out.stderr] {
                let text = String::from_utf8_lossy(stream);
                assert!(!text.contains("DW_PUBLICATION_SENTINEL"));
                assert!(!text.contains("unsupported.example.invalid"));
            }
            assert_eq!(corrupted, context_support::scoped_state(e, c));
        }
    });
    assert_eq!(original, context_support::scoped_state(e, c));
    // Normal preparation works after restore
    context_support::assert_mission_frontier(&context_support::prepare_pack(&q), &source);
    assert_eq!(original, context_support::scoped_state(e, c));
}

fn contract_field(fault: &mut PublicationFault, field: &str, value: Value) {
    let mut contract = fault.contract();
    contract[field] = value;
    fault.set_contract(contract);
}

fn consent_field(fault: &mut PublicationFault, field: &str, value: Value) {
    let mut contract = fault.contract();
    contract["fields"]["m1_inference_permission"][field] = value;
    fault.set_contract(contract);
}

#[test]
fn p01_unknown_outer_field_sql_gate_and_cli_defer() {
    publication_case(
        0xa101,
        Fail::State("context_mission_unavailable"),
        Some("deferred"),
        |fault| {
            // 1. Unknown outer field in contract
            contract_field(
                fault,
                "forged_outer_field",
                json!("DW_PUBLICATION_SENTINEL"),
            );
        },
    );
}

#[test]
fn p02_unknown_inference_consent_field_owner_codec_rejects() {
    for (seed, field, value) in [
        (0xa102, "forged_consent", json!("DW_PUBLICATION_SENTINEL")),
        (0xa122, "terms_approved", json!(false)),
    ] {
        publication_case(seed, Fail::Store("contract_decode"), None, |fault| {
            consent_field(fault, field, value);
        });
    }
}

#[test]
fn p03_catalog_event_identity_sql_gate_rejects() {
    publication_case(
        0xa103,
        Fail::State("context_mission_unavailable"),
        None,
        |fault| {
            fault.set_catalog_event(Uuid::from_u128(0xa203));
        },
    );
}

#[test]
fn p04_contract_event_identity_sql_gate_rejects() {
    publication_case(
        0xa104,
        Fail::State("context_mission_unavailable"),
        None,
        |fault| {
            contract_field(fault, "event_id", json!(Uuid::from_u128(0xa204)));
        },
    );
}

#[test]
fn p05_contract_operation_identity_sql_gate_rejects() {
    publication_case(
        0xa105,
        Fail::State("context_mission_unavailable"),
        None,
        |fault| {
            contract_field(fault, "operation_id", json!(Uuid::from_u128(0xa205)));
        },
    );
}

fn published_basis(fault: &mut PublicationFault, field: &str, value: Uuid) {
    let mut contract = fault.contract();
    contract["fields"][field] = json!(value);
    contract["fields"]["m1_inference_permission"][field] = json!(value);
    fault.set_contract(contract);
}

#[test]
fn p06a_published_goal_differs_from_committed_mission() {
    publication_case(
        0xa106,
        Fail::State("context_mission_unavailable"),
        None,
        |fault| {
            published_basis(fault, "goal_ref", Uuid::from_u128(0xa206));
        },
    );
}

#[test]
fn p06b_published_operator_differs_from_committed_mission() {
    publication_case(
        0xa116,
        Fail::State("context_mission_unavailable"),
        None,
        |fault| {
            published_basis(fault, "operator_ref", Uuid::from_u128(0xa216));
        },
    );
}

#[test]
fn p07_owner_goal_differs_from_unchanged_publication() {
    publication_case(
        0xa107,
        Fail::State("context_mission_unavailable"),
        None,
        |fault| {
            fault.set_owner_goal(Uuid::from_u128(0xa207));
        },
    );
}

#[test]
fn p08_actual_foreign_campaign_publication_rejects_and_preserves_both_scopes() {
    publication_case(
        0xa108,
        Fail::State("context_mission_unavailable"),
        None,
        |fault| {
            let (foreign, _) = budget::fixture(0xa208);
            let (e, c) = (foreign.engagement_id.0, foreign.campaign_id.0);
            let before = context_support::scoped_state(e, c);
            context_support::prepare_pack(&context_support::context_request(&foreign));
            fault.protect_foreign(e, c);
            let mut contract = fault.contract();
            contract["engagement_id"] = json!(foreign.engagement_id);
            contract["campaign_id"] = json!(foreign.campaign_id);
            contract["affected_entity"] = json!(foreign.campaign_id);
            contract["fields"]["m1_inference_permission"]["engagement_id"] =
                json!(foreign.engagement_id);
            contract["fields"]["m1_inference_permission"]["campaign_id"] =
                json!(foreign.campaign_id);
            fault.set_contract(contract);
            assert_eq!(before, context_support::scoped_state(e, c));
        },
    );
}

#[test]
fn p09_wrong_recipient_type_owner_codec_and_cli_unavailable() {
    publication_case(
        0xa109,
        Fail::Store("contract_decode"),
        Some("unavailable"),
        |fault| {
            consent_field(fault, "recipient", json!({"DW_PUBLICATION_SENTINEL": 1}));
        },
    );
}

#[test]
fn p10_unsupported_recipient_owner_qualification_and_cli_unavailable() {
    publication_case(
        0xa110,
        Fail::Store("contract_decode"),
        Some("unavailable"),
        |fault| {
            consent_field(
                fault,
                "recipient",
                json!("https://unsupported.example.invalid/responses"),
            );
        },
    );
}

#[test]
fn p11_wrong_profile_type_owner_codec_rejects() {
    publication_case(0xa111, Fail::Store("contract_decode"), None, |fault| {
        consent_field(fault, "profile", json!({"DW_PUBLICATION_SENTINEL": 1}));
    });
}

#[test]
fn p12_unsupported_profile_owner_qualification_rejects() {
    publication_case(0xa112, Fail::Store("contract_decode"), None, |fault| {
        consent_field(fault, "profile", json!("DW_PUBLICATION_SENTINEL"));
    });
}

#[test]
fn p13_actual_provider_terms_boolean_false_owner_qualification_rejects() {
    publication_case(0xa113, Fail::Store("contract_decode"), None, |fault| {
        let original = fault.contract();
        assert_eq!(
            original["fields"]["m1_inference_permission"]["provider_terms_approved"],
            true
        );
        let mut changed = original.clone();
        changed["fields"]["m1_inference_permission"]["provider_terms_approved"] = json!(false);
        let mut restored = changed.clone();
        restored["fields"]["m1_inference_permission"]["provider_terms_approved"] = json!(true);
        assert_eq!(restored, original);
        fault.set_contract(changed);
    });
}

#[test]
fn controlled_assertion_panic_restores_exact_rows_before_original_panic_resumes() {
    let _g = db_support::db();
    let (req, source) = budget::fixture(0xa114);
    let (e, c) = (req.engagement_id.0, req.campaign_id.0);
    let original = context_support::scoped_state(e, c);
    let fault = PublicationFault::capture(e, c);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fault.checked(|fault| {
            contract_field(
                fault,
                "forged_outer_field",
                json!("DW_PUBLICATION_SENTINEL"),
            );
            assert_eq!(
                context_support::scoped_state(e, c),
                original,
                "controlled publication assertion panic"
            );
        });
    }));
    let panic = result.expect_err("the controlled assertion must panic");
    assert!(
        panic
            .downcast_ref::<String>()
            .unwrap()
            .contains("controlled publication assertion panic")
    );
    assert_eq!(original, context_support::scoped_state(e, c));
    let mut fresh_admin = session_support::admin_db_client();
    let row = fresh_admin.query_one(
        "SELECT to_jsonb(o), to_jsonb(m) FROM mission.registration_outbox o JOIN mission.missions m USING (engagement_id,campaign_id,operation_id) WHERE engagement_id=$1 AND campaign_id=$2",
        &[&e, &c],
    ).unwrap();
    assert_eq!(row.get::<_, Value>(0), original["publication"]);
    assert_eq!(row.get::<_, Value>(1), original["mission"]);
    context_support::assert_mission_frontier(
        &context_support::prepare_pack(&context_support::context_request(&req)),
        &source,
    );
    assert_eq!(original, context_support::scoped_state(e, c));
}
