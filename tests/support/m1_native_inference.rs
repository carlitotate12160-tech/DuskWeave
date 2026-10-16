//! Explicit native fixture configuration; provisioning belongs to the bootstrap.
#![allow(dead_code)]
use postgres::{Client, NoTls};
use serde_json::{Value, json};
use std::sync::{Mutex, MutexGuard, Once};

const VARIABLES: [&str; 5] = [
    "DW_TEST_ADMIN_DATABASE_URL",
    "DW_TEST_DATABASE_URL",
    "DW_INFERENCE_BROKER_DATABASE_URL",
    "DW_COGNITION_DATABASE_URL",
    "DW_TRAJECTORY_DATABASE_URL",
];
static DB: Mutex<()> = Mutex::new(());

fn config(variable: &str) -> postgres::Config {
    std::env::var(variable)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("native_fixture_configuration_invalid"))
}
pub fn client(variable: &str) -> Client {
    config(variable)
        .connect(NoTls)
        .unwrap_or_else(|_| panic!("native_fixture_connection_failed"))
}
pub fn admin() -> Client {
    let mut cfg = config(VARIABLES[0]);
    cfg.dbname(
        config(VARIABLES[1])
            .get_dbname()
            .expect("native_fixture_configuration_invalid"),
    );
    cfg.connect(NoTls)
        .unwrap_or_else(|_| panic!("native_fixture_connection_failed"))
}
pub fn db() -> MutexGuard<'static, ()> {
    let guard = DB.lock().unwrap_or_else(|e| e.into_inner());
    setup();
    guard
}
pub fn setup() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let expected = std::env::var("DW_NATIVE_FIXTURE_DATABASE").expect("native_fixture_configuration_invalid");
        assert!(expected.len() <= 63 && expected.len() > "dw_native_".len()
            && expected.starts_with("dw_native_")
            && expected.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
            "native_fixture_configuration_invalid");
        let configs: Vec<_> = VARIABLES.iter().map(|v|config(v)).collect();
        for (index,cfg) in configs.iter().enumerate() {
            assert!(cfg.get_dbname()==Some(expected.as_str()) || index==0,"native_fixture_configuration_invalid");
            assert!(matches!(cfg.get_hosts(),[postgres::config::Host::Tcp(h)] if h=="127.0.0.1" || h=="localhost"),"native_fixture_configuration_invalid");
            assert!(cfg.get_ports()==[5432] && cfg.get_password().is_some(),"native_fixture_configuration_invalid");
            let user = cfg.get_user().expect("native_fixture_configuration_invalid");
            assert!(!configs[..index].iter().any(|old|old.get_user()==Some(user)),"native_fixture_principals_not_distinct");
        }
        for (variable,group) in [(VARIABLES[1],"dw_runtime"),(VARIABLES[2],"dw_m1_broker"),
                                 (VARIABLES[3],"dw_m1_cognition"),(VARIABLES[4],"dw_m1_trajectory")] {
            let mut c = client(variable);
            let good: bool = c.query_one("SELECT pg_has_role(session_user,$1,'MEMBER') AND NOT EXISTS(SELECT 1 FROM pg_roles r WHERE pg_has_role(session_user,r.oid,'SET') AND (rolsuper OR rolbypassrls OR rolcreatedb OR rolcreaterole)) AND NOT EXISTS(SELECT 1 FROM unnest(ARRAY['dw_m1_broker','dw_m1_cognition','dw_m1_trajectory']) g WHERE g<>$1 AND pg_has_role(session_user,g,'MEMBER')) AND ($1<>'dw_m1_trajectory' OR NOT pg_has_role(session_user,'dw_runtime','MEMBER')) AND current_setting('fsync')='on' AND current_setting('full_page_writes')='on' AND current_setting('synchronous_commit')='on'",&[&group]).unwrap_or_else(|_|panic!("native_fixture_privileges_invalid")).get(0);
            assert!(good,"native_fixture_privileges_invalid");
            {
                let narrow: bool = c.query_one("SELECT NOT EXISTS(SELECT 1 FROM pg_roles a WHERE pg_has_role(session_user,a.oid,'SET') AND (EXISTS(SELECT 1 FROM pg_namespace n WHERE n.nspname IN ('mission','execution','cognition','trajectory') AND (has_schema_privilege(a.oid,n.oid,'CREATE') OR pg_has_role(a.oid,n.nspowner,'MEMBER'))) OR EXISTS(SELECT 1 FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace WHERE n.nspname IN ('mission','execution','cognition','trajectory') AND t.relkind='r' AND (pg_has_role(a.oid,t.relowner,'MEMBER') OR has_table_privilege(a.oid,t.oid,'UPDATE,DELETE,TRUNCATE,TRIGGER') OR has_any_column_privilege(a.oid,t.oid,'UPDATE') OR (has_any_column_privilege(a.oid,t.oid,'INSERT') AND NOT (($1='dw_m1_trajectory' AND n.nspname='trajectory' AND t.relname='reasoning_history') OR ($1='dw_runtime' AND n.nspname IN ('mission','trajectory') AND t.relname<>'reasoning_history')))))))",&[&group]).unwrap_or_else(|_|panic!("native_fixture_privileges_invalid")).get(0);
                assert!(narrow,"native_fixture_privileges_invalid");
            }
        }
        // Legacy CI targets can redefine earlier functions. Requalify this
        // candidate's functions without creating/rotating/granting any login.
        let mut a = admin();
        assert!(a.query_one("SELECT pg_has_role(session_user,datdba,'MEMBER') FROM pg_database WHERE datname=current_database()",&[]).unwrap_or_else(|_|panic!("native_fixture_privileges_invalid")).get::<_,bool>(0),"native_fixture_privileges_invalid");
        for part in include_str!("../../migrations/0012_native_inference.sql").split("\nCREATE FUNCTION ").skip(1) {
            let end = part.find("$$;").expect("native function terminator")+3;
            a.batch_execute(&format!("CREATE OR REPLACE FUNCTION {}",&part[..end])).expect("native function refresh");
        }
        a.batch_execute(&include_str!("../../migrations/0013_inference_profile_and_continuation.sql").replace("CREATE FUNCTION ","CREATE OR REPLACE FUNCTION ")).expect("native function refresh");
    });
}

pub fn bind_request(q: &Value, s: &duskweave::m1_reasoning::inference::Scope) -> String {
    use sha2::{Digest, Sha256};
    let typed: duskweave::m1_context_input::ContextRequest =
        serde_json::from_value(q.clone()).unwrap();
    typed.validate_ingress().unwrap();
    let bytes = serde_json::to_vec(&typed).unwrap();
    let hash: String = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    client("DW_INFERENCE_BROKER_DATABASE_URL")
        .execute(
            "SELECT execution.bind_inference_input($1,$2,$3,$4)",
            &[&s.engagement, &s.campaign, &s.decision, &hash],
        )
        .unwrap();
    hash
}

pub struct NativeCase {
    pub request: Value,
    pub scope: duskweave::m1_reasoning::inference::Scope,
    pub pack: Vec<u8>,
    pub broker: duskweave::m1_reasoning::postgres::PgBroker,
    pub store: duskweave::m1_reasoning::postgres::PgDecisionStore,
    pub history: duskweave::postgres_trajectory::PgTrajectory,
}
pub fn decision(n: u128) -> NativeCase {
    use duskweave::mission::OperationId;
    use duskweave::registration::OperationAllocator;
    let (_, q) = registered(n);
    let typed: duskweave::m1_context_input::ContextRequest =
        serde_json::from_value(q.clone()).unwrap();
    let id = duskweave::postgres_mission::PgAllocator::new(client("DW_TEST_DATABASE_URL"))
        .allocate()
        .unwrap();
    let scope = duskweave::m1_reasoning::inference::Scope {
        engagement: typed.mission.engagement_id.0,
        campaign: typed.mission.campaign_id.0,
        decision: id,
        mission_revision: 1,
        permission: typed.mission.permission_id,
    };
    let hash = bind_request(&q, &scope);
    let pack = duskweave::m1_context::prepare(
        &mut duskweave::postgres_mission::PgContextReader::new(client(
            "DW_INFERENCE_BROKER_DATABASE_URL",
        )),
        &typed,
        OperationId(id),
    )
    .unwrap();
    NativeCase {
        request: q,
        scope,
        pack: pack.utf8().as_bytes().to_vec(),
        broker: duskweave::m1_reasoning::postgres::PgBroker {
            client: client("DW_INFERENCE_BROKER_DATABASE_URL"),
            mission: duskweave::postgres_mission::PgMissionStore::new(client(
                "DW_TEST_DATABASE_URL",
            )),
            reservations: duskweave::postgres_inference_budget::PgInferenceReservationStore::new(
                client("DW_INFERENCE_BROKER_DATABASE_URL"),
            ),
            request: typed.mission,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(20),
        },
        store: duskweave::m1_reasoning::postgres::PgDecisionStore {
            client: client("DW_COGNITION_DATABASE_URL"),
            input_hash: hash,
        },
        history: duskweave::postgres_trajectory::PgTrajectory::new(client(
            "DW_TRAJECTORY_DATABASE_URL",
        )),
    }
}
pub fn through(variable: &str, port: u16) -> Client {
    let source = std::env::var(variable).expect("native_fixture_configuration_invalid");
    assert_eq!(
        source.matches("@127.0.0.1:5432/").count(),
        1,
        "native_fixture_configuration_invalid"
    );
    let routed = source.replacen("@127.0.0.1:5432/", &format!("@127.0.0.1:{port}/"), 1);
    routed
        .parse::<postgres::Config>()
        .unwrap_or_else(|_| panic!("native_fixture_configuration_invalid"))
        .connect(NoTls)
        .unwrap_or_else(|_| panic!("native_fixture_connection_failed"))
}

pub fn registered(n: u128) -> (Value, Value) {
    use duskweave::registration;
    let (e, c) = super::db_support::scope(n);
    let now: i64 = admin()
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .unwrap()
        .get(0);
    let permission = uuid::Uuid::from_u128(e.0.as_u128().wrapping_add(100));
    let limits = json!({"context_bytes":4096,"candidates":8,"request_bytes":16384,"configuration_bytes":16384,
        "response_bytes":65536,"calls_per_decision":2,"output_tokens_per_call":4096,"decision_wall_time_ms":20000,"connect_time_ms":5000,
        "proposal_bytes":8192,"repair_bytes":2048,"text_field_bytes":1024,"repair_error_codes":8,"json_depth":32,"collection_entries":64,"concurrency":1});
    let raw = json!({"engagement_id":e,"campaign_id":c,"operator_ref":uuid::Uuid::from_u128(0x11),
        "authority_ref":uuid::Uuid::from_u128(0x12),"authority_revision":1,"goal_ref":uuid::Uuid::from_u128(0x13),
        "included_assets":[uuid::Uuid::from_u128(0x21)],"excluded_assets":[],"exercise_mode":"blind",
        "starts_at":now-60,"ends_at":now+3600,"m1_inference_permission":{
        "version":"m1_inference_permission_v2","permission_id":permission,"engagement_id":e,"campaign_id":c,
        "operator_ref":uuid::Uuid::from_u128(0x11),"authority_ref":uuid::Uuid::from_u128(0x12),"authority_revision":1,
        "mission_revision":1,"goal_ref":uuid::Uuid::from_u128(0x13),"exercise_mode":"blind",
        "recipient":"https://api.deepseek.com/responses","profile":"deepseek-responses-envelope32-v1",
        "field_classes":["mission_question_constraints","scoped_opaque_references","omissions_unknown","capacity","admitted_operator_premises","admitted_repair_fields_codes"],
        "starts_at":now-30,"ends_at":now+1800,"provider_terms_ref":uuid::Uuid::from_u128(0x32),"provider_terms_approved":true,
        "campaign_limits":{"calls":10,"request_bytes":100000,"response_bytes":500000,"output_token_capacity":20000,"wall_time_ms":100000},
        "operation_limits":limits}});
    let input = duskweave::input::parse_register(&serde_json::to_vec(&raw).unwrap()).unwrap();
    let (mut a, mut m, mut h) = super::db_support::ports();
    let op = registration::prepare_operation(&mut a).unwrap();
    registration::register(&mut a, &mut m, &mut h, op, &input).unwrap();
    let request = json!({"version":"m1_context_prepare_request_v1","mission":{
        "version":"m1_inference_reservation_request_v1","engagement_id":e,"campaign_id":c,"operator_ref":raw["operator_ref"],
        "expected_mission_revision":1,"permission_id":permission,"goal_ref":raw["goal_ref"],"exercise_mode":"blind"},
        "candidates":[{"asset_ref":uuid::Uuid::from_u128(0x21),"priority":"high"}]});
    (raw, request)
}
