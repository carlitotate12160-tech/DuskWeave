//! Extracted owned-upgrade-database fixture: bounded identifier validation,
//! collision denial, OID/owner confirmation and qualified retention. No Drop
//! cleanup; finish() never deletes and every invocation retains its database
//! for reconciliation. Callers close their clients before finish().
#![allow(dead_code)]

use crate::db_support::dsn;
use postgres::{Client, NoTls};

/// Bounded upgrade-database identifier: a safe SQL identifier that cannot
/// name a template, reserved database or inject a statement.
pub fn upgrade_identifier(name: &str) -> Result<(), &'static str> {
    if name.is_empty()
        || name.len() > 63
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        || ["postgres", "template0", "template1"]
            .iter()
            .any(|n| name.eq_ignore_ascii_case(n))
    {
        return Err("invalid_upgrade_identifier");
    }
    Ok(())
}

pub fn admin_in(dbname: &str) -> Client {
    let mut config = dsn("DW_TEST_ADMIN_DATABASE_URL");
    config.dbname(dbname);
    config
        .connect(NoTls)
        .unwrap_or_else(|_| panic!("upgrade admin connection failed"))
}

pub fn runtime_in(dbname: &str) -> Client {
    let mut config = dsn("DW_TEST_DATABASE_URL");
    config.dbname(dbname);
    config
        .connect(NoTls)
        .unwrap_or_else(|_| panic!("upgrade runtime connection failed"))
}

// No Drop cleanup: every invocation retains its database for reconciliation.
pub struct OwnedUpgradeDatabase {
    cluster: Client,
    pub name: String,
    oid: u32,
    owner: u32,
    created: bool,
}

impl OwnedUpgradeDatabase {
    pub fn create() -> Result<Self, &'static str> {
        let runtime = dsn("DW_TEST_DATABASE_URL");
        let primary = runtime.get_dbname().ok_or("missing_primary_identity")?;
        upgrade_identifier(primary)?;
        let name = format!("{primary}_v1_upgrade_{}", std::process::id());
        upgrade_identifier(&name)?;
        let admin = dsn("DW_TEST_ADMIN_DATABASE_URL");
        if name == primary || Some(name.as_str()) == admin.get_dbname() {
            return Err("upgrade_identity_collision");
        }
        let mut cluster = admin_in("postgres");
        if cluster
            .query_opt("SELECT 1 FROM pg_database WHERE datname=$1", &[&name])
            .map_err(|_| "upgrade_collision_check_failed")?
            .is_some()
        {
            return Err("upgrade_identity_collision");
        }
        cluster
            .batch_execute("SET statement_timeout='10s'")
            .map_err(|_| "upgrade_timeout_failed")?;
        cluster
            .batch_execute(&format!("CREATE DATABASE \"{name}\" TEMPLATE template0"))
            .map_err(|_| "upgrade_create_unconfirmed")?;
        let created = true;
        eprintln!("upgrade_database={name} stage=created");
        let row = cluster
            .query_one(
                "SELECT oid,datdba,datdba=(SELECT oid FROM pg_roles WHERE rolname=current_user)              FROM pg_database WHERE datname=$1",
                &[&name],
            )
            .map_err(|_| "upgrade_catalog_failed")?;
        if !row.get::<_, bool>(2) {
            return Err("upgrade_owner_mismatch");
        }
        Ok(Self {
            cluster,
            name,
            oid: row.get(0),
            owner: row.get(1),
            created,
        })
    }

    pub fn finish(mut self) -> Result<(), &'static str> {
        let row = self
            .cluster
            .query_one(
                "SELECT oid,datdba FROM pg_database WHERE datname=$1",
                &[&self.name],
            )
            .map_err(|_| "upgrade_retention_catalog_failed")?;
        if !self.created || row.get::<_, u32>(0) != self.oid || row.get::<_, u32>(1) != self.owner {
            return Err("upgrade_retention_identity_mismatch");
        }
        eprintln!(
            "upgrade_database={} stage=retained oid={} owner={}",
            self.name, self.oid, self.owner
        );
        Ok(())
    }
}
