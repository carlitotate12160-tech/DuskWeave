//! Private support for M1 lease coordination and fence regression assertions.
use super::fixture::*;
use super::receipt::LabBasis;
use super::run_support::*;
use duskweave::m1_reasoning::{
    inference::{Broker, Provider, Received, ReleaseFailure, Reservation, Scope},
    postgres::Basis,
    postgres_terrain::TerrainBasis,
    proposal::ProposalBasis,
    request::{self, BoundCall},
    response,
};
use duskweave::postgres_m1_run::PgRunStore;
use postgres::Client;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub struct CountingRepairProvider {
    pub count: Arc<AtomicUsize>,
    pub releases: Arc<AtomicUsize>,
}

impl Provider for CountingRepairProvider {
    fn prepare(&mut self, call: &BoundCall, _: Instant) -> Result<(), &'static str> {
        let pack = std::str::from_utf8(&call.pack).map_err(|_| "pack_utf8")?;
        let conf = std::str::from_utf8(&call.configuration).map_err(|_| "conf_utf8")?;
        let body = std::str::from_utf8(&call.body).map_err(|_| "body_utf8")?;
        request::verify_binding(
            pack,
            conf,
            body,
            &call.digest,
            self.count.load(Ordering::SeqCst) != 0,
        )
    }

    fn call(
        &mut self,
        call: &BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        let call_idx = self.count.fetch_add(1, Ordering::SeqCst);
        release()?;
        self.releases.fetch_add(1, Ordering::SeqCst);

        let pack: Value = serde_json::from_slice(&call.pack).map_err(|_| "pack_decode")?;
        let mut proposal = super::genai::proposal(&pack);
        if call_idx == 0 {
            // First call (generation): malformed proposal missing required hypothesis
            proposal.as_object_mut().unwrap().remove("hypothesis");
        } else {
            // Second call (repair): synthetic invalid repair to exhaust repair
            proposal["synthetic_unknown_probe"] = serde_json::json!("synthetic_probe_value");
        }
        let raw = serde_json::to_vec(&super::genai::envelope(&proposal))
            .map_err(|_| "envelope_encode")?;
        response::admit_envelope(&raw, raw.len(), Duration::ZERO, basis)
    }
}

pub struct ZeroProvider(pub Arc<AtomicUsize>);

impl Provider for ZeroProvider {
    fn prepare(&mut self, _: &BoundCall, _: Instant) -> Result<(), &'static str> {
        Ok(())
    }
    fn call(
        &mut self,
        _: &BoundCall,
        _: Instant,
        _: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        self.0.fetch_add(1, Ordering::SeqCst);
        release()?;
        Err("provider_error")
    }
}

pub struct HolderBroker<B: Broker> {
    pub inner: B,
    pub run: Uuid,
    pub holder: Option<Client>,
    pub repair_call: Uuid,
    pub renew_executed: bool,
    pub fence_owned_during_renew: bool,
}

impl<B: Broker> HolderBroker<B> {
    pub fn new(inner: B, run: Uuid, holder: Client, repair_call: Uuid) -> Self {
        Self {
            inner,
            run,
            holder: Some(holder),
            repair_call,
            renew_executed: false,
            fence_owned_during_renew: false,
        }
    }
}

impl<B: Broker> Broker for HolderBroker<B> {
    fn reserve(&mut self, scope: &Scope, pack: &[u8]) -> Result<Reservation, &'static str> {
        let res = self.inner.reserve(scope, pack)?;
        self.repair_call = res.repair_call;
        Ok(res)
    }

    fn may_send(
        &mut self,
        scope: &Scope,
        call_id: Uuid,
        bound: &BoundCall,
    ) -> Result<(), &'static str> {
        self.inner.may_send(scope, call_id, bound)?;
        if call_id == self.repair_call && !self.renew_executed {
            let Some(ref mut h) = self.holder else {
                return Ok(());
            };
            h.batch_execute("BEGIN")
                .map_err(|_| "holder_begin_failed")?;
            h.execute("SELECT execution.run_renew($1)", &[&self.run])
                .map_err(|_| "holder_renew_failed")?;
            self.renew_executed = true;
            let pid: i32 = h.query_one("SELECT pg_backend_pid()", &[]).unwrap().get(0);
            let key: i64 = admin()
                .query_one(
                    "SELECT hashtextextended($1,0)",
                    &[&format!("{}:{}", scope.engagement, scope.campaign)],
                )
                .unwrap()
                .get(0);
            let holds: bool = admin()
                .query_one(
                    "SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND pid=$1 AND (((classid::bigint)<<32)|objid::bigint)=$2)",
                    &[&pid, &key],
                )
                .unwrap()
                .get(0);
            self.fence_owned_during_renew = holds;
        }
        Ok(())
    }

    fn release_into(
        &mut self,
        scope: &Scope,
        reservation: &Reservation,
        call_id: Uuid,
        bound: &BoundCall,
        provider: &mut dyn Provider,
        basis: &mut dyn ProposalBasis,
    ) -> Result<Received, ReleaseFailure> {
        let result = self
            .inner
            .release_into(scope, reservation, call_id, bound, provider, basis);
        if call_id == self.repair_call {
            let Some(mut h) = self.holder.take() else {
                return result;
            };
            h.batch_execute("COMMIT")
                .expect("holder commit must succeed before outer settlement");
        }
        result
    }

    fn never_released(
        &mut self,
        scope: &Scope,
        call_id: Uuid,
        category: &'static str,
    ) -> Result<(), &'static str> {
        self.inner.never_released(scope, call_id, category)
    }

    fn completed(
        &mut self,
        scope: &Scope,
        call_id: Uuid,
        usage: &duskweave::m1_reasoning::inference::Usage,
    ) -> Result<(), &'static str> {
        self.inner.completed(scope, call_id, usage)
    }

    fn completed_with_diagnostic(
        &mut self,
        scope: &Scope,
        call_id: Uuid,
        usage: &duskweave::m1_reasoning::inference::Usage,
        diag: Option<&duskweave::m1_reasoning::proposal_diagnostic::ProposalDiagnostic>,
    ) -> Result<(), &'static str> {
        self.inner
            .completed_with_diagnostic(scope, call_id, usage, diag)
    }

    fn uncertain(
        &mut self,
        scope: &Scope,
        call_id: Uuid,
        category: &'static str,
    ) -> Result<(), &'static str> {
        self.inner.uncertain(scope, call_id, category)
    }

    fn uncertain_with_report(
        &mut self,
        scope: &Scope,
        call_id: Uuid,
        category: &'static str,
        report: Option<&duskweave::m1_reasoning::inference::ProviderReport>,
    ) -> Result<(), &'static str> {
        self.inner
            .uncertain_with_report(scope, call_id, category, report)
    }

    fn close_unused_repair(
        &mut self,
        scope: &Scope,
        reservation: &Reservation,
    ) -> Result<(), &'static str> {
        self.inner.close_unused_repair(scope, reservation)
    }

    fn provenance(&mut self, scope: &Scope) -> Result<Value, &'static str> {
        self.inner.provenance(scope)
    }

    fn finish(&mut self, scope: &Scope) -> Result<(), &'static str> {
        self.inner.finish(scope)
    }

    fn inspect(&mut self, scope: &Scope) -> Result<&'static str, &'static str> {
        self.inner.inspect(scope)
    }

    fn inspect_pack(&mut self, scope: &Scope) -> Result<Option<String>, &'static str> {
        self.inner.inspect_pack(scope)
    }
}

impl<B: Broker> Drop for HolderBroker<B> {
    fn drop(&mut self) {
        if let Some(mut h) = self.holder.take() {
            let _ = h.batch_execute("ROLLBACK");
        }
    }
}

pub fn fence_key(e: Uuid, c: Uuid) -> i64 {
    admin()
        .query_one("SELECT hashtextextended($1,0)", &[&format!("{e}:{c}")])
        .unwrap()
        .get(0)
}

pub fn holder_holds_fence(pid: i32, key: i64) -> bool {
    admin()
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND pid=$1 AND (((classid::bigint)<<32)|objid::bigint)=$2)",
            &[&pid, &key],
        )
        .unwrap()
        .get(0)
}

pub fn other_broker() -> Client {
    client("DW_NATIVE_OTHER_BROKER_DATABASE_URL")
}

pub fn terrain_column() -> Client {
    client("DW_NATIVE_TERRAINCOLUMN_DATABASE_URL")
}

pub fn wait_for_blocking_pid(
    observer: &mut Client,
    blocked_pid: i32,
    blocking_pid: i32,
    timeout: Duration,
) -> Result<(), &'static str> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let is_blocked: bool = observer
            .query_one(
                "SELECT $1 = ANY(pg_blocking_pids($2))",
                &[&blocking_pid, &blocked_pid],
            )
            .map_err(|_| "query_blocking_failed")?
            .get(0);
        if is_blocked {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Err("timeout_waiting_for_row_lock")
}

pub fn setup_test_mutator(allowed_runs: &[Uuid], allowed_roles: &[&str]) {
    let mut a = admin();
    let run_literals = allowed_runs
        .iter()
        .map(|u| format!("'{u}'::uuid"))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "CREATE OR REPLACE FUNCTION execution.test_mutate_lease(r uuid, new_lease bigint, new_status text) \
         RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog, pg_temp AS $$ \
         BEGIN \
          IF r <> ALL(ARRAY[{run_literals}]) THEN \
           RAISE EXCEPTION 'test_mutator_scope_refused'; \
          END IF; \
          UPDATE execution.autonomous_control \
           SET lease_until_ms=new_lease, status=coalesce(new_status, status) \
           WHERE run_id=r; \
         END $$; \
         REVOKE ALL ON FUNCTION execution.test_mutate_lease(uuid,bigint,text) FROM PUBLIC; \
         REVOKE ALL ON FUNCTION execution.test_mutate_lease(uuid,bigint,text) FROM dw_m1_broker;"
    );
    a.batch_execute(&sql).unwrap();
    for role in allowed_roles {
        a.batch_execute(&format!(
            "GRANT USAGE ON SCHEMA execution TO \"{role}\"; \
             GRANT EXECUTE ON FUNCTION execution.test_mutate_lease(uuid,bigint,text) TO \"{role}\";"
        ))
        .unwrap();
    }
}

pub fn assert_direct_lock_fences_release(slot: u128) {
    let (ctrl_e, ctrl_c, _) = register_live(slot, false);
    let ctrl_run = Uuid::new_v4();
    let ctrl_pub = live_publication(ctrl_e, ctrl_c, ctrl_run);
    let mut ctrl_runs = PgRunStore::new(broker());
    ctrl_runs
        .start(
            &successor_request(&ctrl_pub, ctrl_run, "m1_autonomous_run_request_v3"),
            RELEASE,
        )
        .unwrap();
    let ctrl_rec = ctrl_runs.inspect(ctrl_run).unwrap();
    let ctrl_prep = prepare_decision_one(&mut ctrl_runs, &ctrl_rec);
    let ctrl_dead = Instant::now() + Duration::from_millis(ctrl_prep.window_ms as u64);
    let ctrl_pack = ctrl_prep.pack.utf8().as_bytes();
    let mut ctrl_port = broker_port(&ctrl_rec, ctrl_dead);
    let ctrl_res = ctrl_port.reserve(&ctrl_prep.scope, ctrl_pack).unwrap();
    let ctrl_bnd = duskweave::m1_reasoning::inference_intent::bind(
        &ctrl_prep.scope,
        &ctrl_res,
        ctrl_res.generation_call,
        ctrl_pack,
        None,
    )
    .unwrap();
    ctrl_port
        .may_send(&ctrl_prep.scope, ctrl_res.generation_call, &ctrl_bnd)
        .unwrap();

    let ctrl_key = fence_key(ctrl_e, ctrl_c);
    let mut direct_tx = admin();
    direct_tx.batch_execute("BEGIN").unwrap();
    direct_tx
        .execute("SELECT pg_advisory_xact_lock($1)", &[&ctrl_key])
        .unwrap();

    let mut ctrl_basis = LabBasis(TerrainBasis {
        mission: Basis {
            client: broker(),
            scope: ctrl_prep.scope.clone(),
            references: request::reference_ids(ctrl_pack).unwrap(),
            deadline: ctrl_dead,
        },
        dependencies: ctrl_prep.pack.terrain_dependencies().to_vec(),
    });
    let ctrl_calls = Arc::new(AtomicUsize::new(0));
    let res = ctrl_port.release_into(
        &ctrl_prep.scope,
        &ctrl_res,
        ctrl_res.generation_call,
        &ctrl_bnd,
        &mut ZeroProvider(ctrl_calls.clone()),
        &mut ctrl_basis,
    );
    assert!(matches!(
        res,
        Err(ReleaseFailure::NeverReleased(
            "current_authority_unavailable"
        ))
    ));
    assert_eq!(ctrl_calls.load(Ordering::SeqCst), 0);
    direct_tx.batch_execute("COMMIT").unwrap();
}
