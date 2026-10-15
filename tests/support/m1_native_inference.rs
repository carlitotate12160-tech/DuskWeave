//! Fresh native owners; never invokes the legacy cluster-role fixtures.
#![allow(dead_code)]
use postgres::{Client, NoTls};
use std::sync::Once;

pub fn admin() -> Client {
    let mut c = crate::db_support::dsn("DW_TEST_ADMIN_DATABASE_URL");
    c.dbname(
        crate::db_support::dsn("DW_TEST_DATABASE_URL")
            .get_dbname()
            .unwrap(),
    );
    c.connect(NoTls).expect("native fixture admin connection")
}
pub fn client(variable: &str) -> Client {
    crate::db_support::dsn(variable)
        .connect(NoTls)
        .expect("native owner connection")
}
pub fn setup() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let mut a = admin();
        if !a.query_one("SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname='cognition')",&[]).unwrap().get::<_,bool>(0) {
            a.batch_execute(include_str!("../../migrations/0012_native_inference.sql")).expect("native migration 12");
        } else {
            // Requalify changed functions on this retained disposable fixture;
            // preserve the RED rows and every earlier database/role.
            for part in include_str!("../../migrations/0012_native_inference.sql").split("\nCREATE FUNCTION ").skip(1) {
                let end = part.find("$$;").expect("native function terminator") + 3;
                a.batch_execute(&format!("CREATE OR REPLACE FUNCTION {}",&part[..end])).expect("native function refresh");
            }
            if !a.query_one("SELECT EXISTS(SELECT 1 FROM pg_trigger WHERE tgname='reasoning_source_guard')",&[]).unwrap().get::<_,bool>(0) {
                a.batch_execute("CREATE TRIGGER reasoning_source_guard BEFORE INSERT ON trajectory.reasoning_history FOR EACH ROW EXECUTE FUNCTION trajectory.guard_reasoning_source()").expect("native history source guard");
            }
        }
        a.batch_execute(&include_str!("../../migrations/0013_inference_profile_and_continuation.sql").replace("CREATE FUNCTION ","CREATE OR REPLACE FUNCTION ")).expect("native migration 13");
        for (variable, expected, groups) in [
            (
                "DW_INFERENCE_BROKER_DATABASE_URL",
                "dw_native_broker_20261015",
                "dw_runtime,dw_m1_broker",
            ),
            (
                "DW_COGNITION_DATABASE_URL",
                "dw_native_cognition_20261015",
                "dw_m1_cognition",
            ),
            (
                "DW_TRAJECTORY_DATABASE_URL",
                "dw_native_history_20261015",
                "dw_m1_trajectory",
            ),
        ] {
            if std::env::var(variable)
                .map(|v| v.trim().is_empty())
                .unwrap_or(true)
            {
                let base = crate::db_support::dsn("DW_TEST_DATABASE_URL");
                let host = match base.get_hosts().first() {
                    Some(postgres::config::Host::Tcp(h)) => h.clone(),
                    _ => "127.0.0.1".to_string(),
                };
                let port = base.get_ports().first().copied().unwrap_or(5432);
                let db = base.get_dbname().unwrap_or("dw_test");
                let pw = "ci-native-role-pw-20261015";
                let synthesized = format!("postgresql://{expected}:{pw}@{host}:{port}/{db}");
                unsafe {
                    std::env::set_var(variable, &synthesized);
                }
            }
            let cfg = crate::db_support::dsn(variable);
            assert_eq!(cfg.get_user(), Some(expected));
            let password = std::str::from_utf8(cfg.get_password().unwrap())
                .unwrap()
                .replace('\'', "''");
            let dbname = cfg.get_dbname().unwrap_or("dw_test");
            if a.query_opt("SELECT 1 FROM pg_roles WHERE rolname=$1", &[&expected])
                .unwrap()
                .is_none()
            {
                a.batch_execute(&format!(
                    "CREATE ROLE {expected} LOGIN NOSUPERUSER NOBYPASSRLS PASSWORD '{password}'; GRANT {groups} TO {expected}; GRANT CONNECT ON DATABASE {dbname} TO {expected};"
                ))
                .expect("fresh native role creation");
            } else {
                a.batch_execute(&format!(
                    "ALTER ROLE {expected} WITH PASSWORD '{password}'; GRANT {groups} TO {expected}; GRANT CONNECT ON DATABASE {dbname} TO {expected};"
                ))
                .expect("refresh native role");
            }
            let _: Client = cfg.connect(NoTls).expect("native role authentication");
        }
    });
}
