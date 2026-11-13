//! Private support for M1 lease coordination and fence regression assertions.
use super::fixture::{admin, client};
use duskweave::m1_reasoning::{
    inference::{Broker, Provider, Received, ReleaseFailure, Reservation, Scope},
    proposal::ProposalBasis,
    request::{self, BoundCall},
    response,
};
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
            let _ = self.holder.take().map(|mut h| h.batch_execute("COMMIT"));
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

pub fn setup_test_mutator() {
    let mut a = admin();
    a.batch_execute(
        "CREATE OR REPLACE FUNCTION execution.test_mutate_lease(r uuid, new_lease bigint, new_status text) \
         RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog, pg_temp AS $$ \
         BEGIN \
          UPDATE execution.autonomous_control \
           SET lease_until_ms=new_lease, status=coalesce(new_status, status) \
           WHERE run_id=r; \
         END $$; \
         REVOKE ALL ON FUNCTION execution.test_mutate_lease(uuid,bigint,text) FROM PUBLIC; \
         GRANT EXECUTE ON FUNCTION execution.test_mutate_lease(uuid,bigint,text) TO dw_m1_broker;",
    )
    .unwrap();
}
