#![allow(dead_code)]
use duskweave::mission::{CampaignId, EngagementId};
use serde_json::{Value, json};
use uuid::Uuid;

pub fn id() -> Uuid {
    crate::db_support::runtime_client()
        .query_one("SELECT gen_random_uuid()", &[])
        .unwrap()
        .get(0)
}

pub fn input(e: EngagementId, c: CampaignId) -> duskweave::mission::RegistrationInput {
    duskweave::input::parse_register(&serde_json::to_vec(&registration(e, c)).unwrap()).unwrap()
}

pub fn admin() -> postgres::Client {
    let mut config = crate::db_support::dsn("DW_TEST_ADMIN_DATABASE_URL");
    config.dbname(
        crate::db_support::dsn("DW_TEST_DATABASE_URL")
            .get_dbname()
            .unwrap(),
    );
    config
        .connect(postgres::NoTls)
        .expect("owned fixture admin connection")
}

pub fn cli(args: &[&str]) -> std::process::Output {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_duskweave"));
    cmd.env_clear()
        .env("DW_DATABASE_CONFIG_MODE", "env-local")
        .env(
            "DW_DATABASE_URL",
            std::env::var("DW_TEST_DATABASE_URL").expect("runtime DSN required"),
        );
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        cmd.env("LLVM_PROFILE_FILE", profile);
    }
    cmd.args(args).output().expect("native CLI process")
}

pub fn text(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

pub fn committed(e: EngagementId, c: CampaignId) -> duskweave::mission::MissionRegistered {
    use duskweave::registration::MissionStore;
    let (mut a, mut s, mut t) = crate::db_support::ports();
    let op = duskweave::registration::prepare_operation(&mut a).unwrap();
    duskweave::registration::register(&mut a, &mut s, &mut t, op, &input(e, c)).unwrap();
    s.outbox_event(e, c, op).unwrap().unwrap()
}

pub fn assert_rows(e: EngagementId, c: CampaignId, history: i64) {
    for (table, n) in [
        ("mission.missions", 1),
        ("mission.registration_outbox", 1),
        ("trajectory.registration_history", history),
    ] {
        assert_eq!(crate::db_support::count(table, e, c), n, "{table}");
    }
}
pub fn registration(e: EngagementId, c: CampaignId) -> Value {
    let operations = json!({"context_bytes":4096,"candidates":8,"request_bytes":16384,
                "configuration_bytes":16384,"response_bytes":65536,"calls_per_decision":2,
                "output_tokens_per_call":4096,"decision_wall_time_ms":20000,
                "connect_time_ms":5000,"proposal_bytes":8192,"repair_bytes":2048,
                "text_field_bytes":1024,"repair_error_codes":8,"json_depth":8,
                "collection_entries":64,"concurrency":1});
    let permission = json!({
        "version":"m1_inference_permission_v1", "permission_id":Uuid::from_u128(0x31),
        "engagement_id":e, "campaign_id":c, "operator_ref":Uuid::from_u128(0x11),
        "authority_ref":Uuid::from_u128(0x12), "authority_revision":1,
        "mission_revision":1, "goal_ref":Uuid::from_u128(0x13),
        "exercise_mode":"blind", "recipient":"https://api.deepseek.com/responses",
        "profile":"deepseek-responses-v1",
        "field_classes":["mission_question_constraints","scoped_opaque_references",
            "omissions_unknown","capacity"],
        "starts_at":1700000001, "ends_at":1700086399,
        "provider_terms_ref":Uuid::from_u128(0x32), "provider_terms_approved":true,
        "campaign_limits":{"calls":10,"request_bytes":100000,"response_bytes":500000,
            "output_token_capacity":20000,"wall_time_ms":100000},
        "operation_limits":operations
    });
    json!({
        "engagement_id":e, "campaign_id":c, "operator_ref":Uuid::from_u128(0x11),
        "authority_ref":Uuid::from_u128(0x12), "authority_revision":1,
        "goal_ref":Uuid::from_u128(0x13), "included_assets":[Uuid::from_u128(0x21)],
        "excluded_assets":[], "exercise_mode":"blind",
        "starts_at":1700000000, "ends_at":1700086400,
        "m1_inference_permission":permission
    })
}
