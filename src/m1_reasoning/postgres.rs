//! Broker and Cognition adapters over existing Mission/reservation ports.
use super::inference::{
    Broker, Decision, DecisionStore, Provider, Received, ReleaseFailure, Reservation, Scope, Usage,
};
use super::proposal::ProposalBasis;
use super::request::BoundCall;
use crate::m1_inference_budget::{self, ReservationAction, ReservationRequest};
use crate::mission::OperationId;
use crate::postgres_inference_budget::PgInferenceReservationStore;
use crate::postgres_mission::PgMissionStore;
use postgres::Client;
use serde_json::Value;
use std::time::Instant;
use uuid::Uuid;
type Res<T> = Result<T, &'static str>;
pub struct PgBroker {
    pub client: Client,
    pub mission: PgMissionStore,
    pub reservations: PgInferenceReservationStore,
    pub request: ReservationRequest,
    pub deadline: Instant,
}
impl PgBroker {
    fn state(&mut self, id: Uuid, state: &str, usage: Option<&Usage>) -> Res<()> {
        let u = serde_json::to_value(usage).map_err(|_| "inference_usage_encode")?;
        self.client
            .query_one(
                "SELECT execution.inference_call_state($1,$2,$3)",
                &[&id, &state, &u],
            )
            .map_err(|_| "inference_accounting_unknown")?;
        Ok(())
    }

    fn verify_release_current(&mut self, s: &Scope, basis: &mut dyn ProposalBasis) -> Res<()> {
        self.client
            .query_one(
                "SELECT execution.inference_release_current($1,$2,$3)",
                &[&s.engagement, &s.campaign, &s.decision],
            )
            .map_err(|_| "current_authority_unavailable")?;
        basis.before_provider_release()
    }
}
impl Broker for PgBroker {
    fn reserve(&mut self, s: &Scope, _: &[u8]) -> Res<Reservation> {
        let r = m1_inference_budget::reservation(
            &mut self.reservations,
            Some(&mut self.mission),
            ReservationAction::Reserve,
            &self.request,
            OperationId(s.decision),
        )
        .map_err(|_| "inference_reservation_unknown")?
        .ok_or("inference_reservation_missing")?;
        Ok(Reservation {
            generation: r.generation as u64,
            generation_call: r.generation_call_id,
            repair_call: r.repair_call_id,
            shared_deadline: self.deadline,
        })
    }
    fn may_send(&mut self, s: &Scope, k: Uuid, b: &BoundCall) -> Res<()> {
        let p = std::str::from_utf8(&b.pack).map_err(|_| "pack_utf8")?;
        let c = std::str::from_utf8(&b.configuration).map_err(|_| "request_encode")?;
        let body = std::str::from_utf8(&b.body).map_err(|_| "request_encode")?;
        let mut tx = self
            .client
            .transaction()
            .map_err(|_| "inference_marker_unknown")?;
        tx.query_one(
            "SELECT execution.inference_may_send($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &s.engagement,
                &s.campaign,
                &s.decision,
                &k,
                &p,
                &c,
                &body,
                &b.digest,
            ],
        )
        .map_err(|_| "inference_marker_refused")?;
        tx.commit().map_err(|_| "inference_marker_commit_unknown")
    }
    fn release_into(
        &mut self,
        s: &Scope,
        r: &Reservation,
        _: Uuid,
        b: &BoundCall,
        p: &mut dyn Provider,
        basis: &mut dyn ProposalBasis,
    ) -> Result<Received, ReleaseFailure> {
        if Instant::now() >= r.shared_deadline {
            return Err(ReleaseFailure::NeverReleased("inference_deadline"));
        }
        // Session lock spans the final SQL check and transport release, never a transaction.
        let acquired: bool = self
            .client
            .query_one(
                "SELECT pg_try_advisory_lock(hashtextextended($1,0))",
                &[&format!("{}:{}", s.engagement, s.campaign)],
            )
            .map_err(|_| ReleaseFailure::NeverReleased("current_authority_unavailable"))?
            .get(0);
        if !acquired {
            // A refused try-lock owns nothing; no cleanup may free another's fence.
            return Err(ReleaseFailure::NeverReleased(
                "current_authority_unavailable",
            ));
        }
        if let Err(code) = self.verify_release_current(s, basis) {
            let _ = self
                .client
                .query_one("SELECT pg_advisory_unlock_all()", &[]);
            return Err(ReleaseFailure::NeverReleased(code));
        }
        if Instant::now() >= r.shared_deadline {
            let _ = self
                .client
                .query_one("SELECT pg_advisory_unlock_all()", &[]);
            return Err(ReleaseFailure::NeverReleased("inference_deadline"));
        }
        let mut released = false;
        let mut release = || {
            self.client
                .query_one("SELECT pg_advisory_unlock_all()", &[])
                .map_err(|_| "inference_release_unknown")?;
            released = true;
            Ok(())
        };
        let result = p.call(b, r.shared_deadline, basis, &mut release);
        // Drop/connection loss also releases the session lock. No replay on any ambiguity.
        if !released {
            let _ = self
                .client
                .query_one("SELECT pg_advisory_unlock_all()", &[]);
        }
        result.map_err(ReleaseFailure::MayHaveSent)
    }
    fn never_released(&mut self, _: &Scope, id: Uuid, _: &'static str) -> Res<()> {
        self.state(id, "failed_never_released", None)
    }
    fn completed(&mut self, _: &Scope, id: Uuid, u: &Usage) -> Res<()> {
        self.state(id, "completed", Some(u))
    }
    fn uncertain(&mut self, _: &Scope, id: Uuid, _: &'static str) -> Res<()> {
        self.state(id, "usage_unknown", None)
    }
    fn uncertain_with_report(
        &mut self,
        _: &Scope,
        id: Uuid,
        _: &'static str,
        report: Option<&super::inference::ProviderReport>,
    ) -> Res<()> {
        let u = serde_json::to_value(report).map_err(|_| "inference_usage_encode")?;
        self.client
            .query_one(
                "SELECT execution.inference_call_state($1,$2,$3)",
                &[&id, &"usage_unknown", &u],
            )
            .map_err(|_| "inference_accounting_unknown")?;
        Ok(())
    }
    fn close_unused_repair(&mut self, _: &Scope, _: &Reservation) -> Res<()> {
        Ok(())
    }
    fn provenance(&mut self, s: &Scope) -> Res<Value> {
        self.client.query_one("SELECT coalesce(jsonb_agg(jsonb_build_object('binding',binding,'requested',configuration::jsonb,'reported',usage,'state',state) ORDER BY call),'[]'::jsonb) FROM execution.inference_calls WHERE e=$1 AND c=$2 AND d=$3",&[&s.engagement,&s.campaign,&s.decision]).map(|r|r.get(0)).map_err(|_|"inference_inspect_unavailable")
    }
    fn finish(&mut self, s: &Scope) -> Res<()> {
        self.client
            .query_one(
                "SELECT execution.close_inference($1,$2,$3)",
                &[&s.engagement, &s.campaign, &s.decision],
            )
            .map_err(|_| "inference_completion_pending")?;
        Ok(())
    }
    fn inspect(&mut self, s: &Scope) -> Res<&'static str> {
        consumption(&mut self.client, s)
    }
    fn inspect_pack(&mut self, s: &Scope) -> Res<Option<String>> {
        inspect_stored_pack(&mut self.client, s)
    }
}
pub fn consumption(c: &mut Client, s: &Scope) -> Res<&'static str> {
    let r = c
        .query_one(
            "SELECT execution.inference_consumption($1,$2,$3)",
            &[&s.engagement, &s.campaign, &s.decision],
        )
        .map_err(|_| "inference_inspect_unavailable")?;
    const STATES: &[&str] = &[
        "uncertain_charged",
        "completed_charged_unused_repair_reserved",
        "failed_never_released_reserved",
        "reserved_not_sent",
        "absent_not_proof_of_no_send",
    ];
    STATES
        .iter()
        .copied()
        .find(|state| *state == r.get::<_, &str>(0))
        .ok_or("inference_decision_integrity")
}

pub struct PgDecisionStore {
    pub client: Client,
    pub input_hash: String,
}
impl DecisionStore for PgDecisionStore {
    fn publish_atomic(&mut self, d: &Decision) -> Res<()> {
        let v = serde_json::to_value(d).map_err(|_| "inference_decision_encode")?;
        let mut tx = self
            .client
            .transaction()
            .map_err(|_| "inference_decision_unknown")?;
        tx.query_one(
            "SELECT cognition.publish_reasoning($1,$2)",
            &[&v, &self.input_hash],
        )
        .map_err(|_| "inference_decision_unknown")?;
        tx.commit().map_err(|_| "inference_decision_commit_unknown")
    }
    fn inspect(&mut self, s: &Scope) -> Res<Option<Decision>> {
        self.client
            .query_opt(
                "SELECT payload FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2 AND d=$3",
                &[&s.engagement, &s.campaign, &s.decision],
            )
            .map_err(|_| "inference_decision_unavailable")?
            .map(|r| decode(r.get(0)))
            .transpose()
    }
}
mod stored_decision;
pub use stored_decision::decode;

// Cognition premise admission over its immutable safe ContextPack.
pub struct Basis {
    pub client: Client,
    pub scope: Scope,
    pub references: Vec<String>,
    pub deadline: Instant,
}
impl ProposalBasis for Basis {
    fn reference_eligible(&self, r: &str) -> bool {
        self.references.iter().any(|v| v == r)
    }
    fn manifest_eligible(&self, _: &str) -> bool {
        false
    }
    fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
        if Instant::now() >= self.deadline {
            return Err("inference_deadline");
        }
        self.client
            .query_one(
                "SELECT execution.inference_release_current($1,$2,$3)",
                &[
                    &self.scope.engagement,
                    &self.scope.campaign,
                    &self.scope.decision,
                ],
            )
            .map_err(|_| "current_authority_unavailable")?;
        Ok(())
    }
    fn safe_explanation(&self, text: &str) -> bool {
        // The opaque generated ContextPack admits no real-client free text in this slice.
        let lower = text.to_ascii_lowercase();
        !text.chars().any(|c| c.is_control())
            && ![
                "sentinel",
                "bearer ",
                "password",
                "api_key",
                "private key",
                "http://",
                "https://",
                "@",
            ]
            .iter()
            .any(|s| lower.contains(s))
    }
}

struct StoredCall {
    call: Uuid,
    binding: String,
    p: String,
    cfg: String,
    body: String,
}

fn inspect_stored_pack(c: &mut Client, s: &Scope) -> Res<Option<String>> {
    let calls = load_stored_calls(c, s)?;
    if calls.is_empty() {
        verify_decision_bindings(c, s, &[], None)?;
        return Ok(None);
    }
    let (gen_id, rep_id) = load_reservation_call_ids(c, s)?;
    let (pack, bindings) = verify_stored_calls(&calls, gen_id, rep_id)?;
    verify_decision_bindings(c, s, &bindings, Some(&[gen_id, rep_id]))?;
    Ok(Some(pack))
}

fn load_stored_calls(c: &mut Client, s: &Scope) -> Res<Vec<StoredCall>> {
    let rows = c
        .query(
            "SELECT call, binding, p, configuration, body \
             FROM execution.inference_calls WHERE e=$1 AND c=$2 AND d=$3 LIMIT 3",
            &[&s.engagement, &s.campaign, &s.decision],
        )
        .map_err(|_| "inference_inspect_unavailable")?;
    Ok(rows
        .into_iter()
        .map(|r| StoredCall {
            call: r.get(0),
            binding: r.get(1),
            p: r.get(2),
            cfg: r.get(3),
            body: r.get(4),
        })
        .collect())
}

fn load_reservation_call_ids(c: &mut Client, s: &Scope) -> Res<(Uuid, Uuid)> {
    let row = c
        .query_opt(
            "SELECT generation_call_id, repair_call_id FROM execution.inference_reservations \
             WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3 AND record_revision=1",
            &[&s.engagement, &s.campaign, &s.decision],
        )
        .map_err(|_| "inference_inspect_unavailable")?
        .ok_or("inference_decision_integrity")?;
    Ok((row.get(0), row.get(1)))
}

fn verify_stored_calls(
    calls: &[StoredCall],
    gen_id: Uuid,
    rep_id: Uuid,
) -> Res<(String, Vec<String>)> {
    if calls.len() > 2 {
        return Err("inference_decision_integrity");
    }
    let gen_call = calls
        .iter()
        .find(|c| c.call == gen_id)
        .ok_or("inference_decision_integrity")?;
    verify_call_bytes(gen_call, false)?;
    let mut bindings = vec![gen_call.binding.clone()];
    if calls.len() == 2 {
        let rep_call = verify_repair_call(calls, gen_id, rep_id, &gen_call.p)?;
        bindings.push(rep_call.binding.clone());
    }
    Ok((gen_call.p.clone(), bindings))
}

fn verify_repair_call<'a>(
    calls: &'a [StoredCall],
    gen_id: Uuid,
    rep_id: Uuid,
    gen_pack: &str,
) -> Res<&'a StoredCall> {
    let rep_call = calls
        .iter()
        .find(|c| c.call == rep_id && c.call != gen_id)
        .ok_or("inference_decision_integrity")?;
    if rep_call.p != gen_pack {
        return Err("inference_decision_integrity");
    }
    verify_call_bytes(rep_call, true)?;
    Ok(rep_call)
}

fn verify_call_bytes(c: &StoredCall, expected_repair: bool) -> Res<()> {
    if super::inference_intent::sdk_pack(c.p.as_bytes()) {
        let intent: Value =
            serde_json::from_str(&c.cfg).map_err(|_| "inference_decision_integrity")?;
        if intent["call_id"] != serde_json::json!(c.call) {
            return Err("inference_decision_integrity");
        }
    }
    super::request::verify_binding(&c.p, &c.cfg, &c.body, &c.binding, expected_repair)
        .map_err(|_| "inference_decision_integrity")
}

fn verify_decision_bindings(
    c: &mut Client,
    s: &Scope,
    expected_bindings: &[String],
    expected_calls: Option<&[Uuid]>,
) -> Res<()> {
    let Some(row) = c
        .query_opt(
            "SELECT payload FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2 AND d=$3",
            &[&s.engagement, &s.campaign, &s.decision],
        )
        .map_err(|_| "inference_inspect_unavailable")?
    else {
        return Ok(());
    };
    let dec = decode(row.get(0))?;
    if !decision_bindings_match(&dec, expected_bindings, expected_calls) {
        return Err("inference_decision_integrity");
    }
    Ok(())
}

fn decision_bindings_match(
    dec: &Decision,
    expected_bindings: &[String],
    expected_calls: Option<&[Uuid]>,
) -> bool {
    dec.bindings == expected_bindings
        && (dec.bindings.is_empty() || Some(dec.call_ids.as_slice()) == expected_calls)
}

const QUALIFY_OWNER_SQL: &str = "SELECT pg_has_role(session_user,$1,'MEMBER')
 AND NOT EXISTS(SELECT 1 FROM unnest(ARRAY['dw_runtime','dw_m1_broker','dw_m1_cognition','dw_m1_trajectory','dw_m1_terrain']) sibling
  WHERE sibling<>$1 AND pg_has_role(session_user,sibling,'SET'))
 AND current_setting('fsync')='on' AND current_setting('full_page_writes')='on'
 AND current_setting('synchronous_commit')='on'
 AND NOT EXISTS(SELECT 1 FROM pg_roles actor
  WHERE pg_has_role(session_user,actor.oid,'SET') AND (
   actor.rolsuper OR actor.rolbypassrls OR actor.rolcreaterole OR actor.rolcreatedb
   OR EXISTS(SELECT 1 FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
    WHERE has_function_privilege(actor.oid,p.oid,'EXECUTE') AND
    ((n.nspname='cognition' AND p.proname='publish_reasoning' AND $1<>'dw_m1_cognition') OR
     (n.nspname='execution' AND p.proname IN ('bind_inference_input','inference_reservation','inference_may_send','inference_call_state','close_inference') AND $1<>'dw_m1_broker') OR
     (n.nspname='terrain' AND p.proname='current_dns_context_view' AND $1<>'dw_m1_broker') OR
     (n.nspname='terrain' AND p.proname IN ('accept_dns_observation','record_dns_integrity_conflict') AND $1<>'dw_m1_terrain')))
   OR EXISTS(SELECT 1 FROM pg_namespace n
    WHERE n.nspname IN ('mission','trajectory','execution','cognition','terrain')
    AND (has_schema_privilege(actor.oid,n.oid,'CREATE')
     OR pg_has_role(actor.oid,n.nspowner,'MEMBER')))
   OR EXISTS(SELECT 1 FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace
    WHERE n.nspname='terrain' AND t.relkind IN ('r','v')
    AND has_any_column_privilege(actor.oid,t.oid,'SELECT')
    AND $1 NOT IN ('dw_m1_terrain','dw_m1_trajectory'))
   OR EXISTS(SELECT 1 FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace
    WHERE n.nspname IN ('mission','trajectory','execution','cognition','terrain') AND t.relkind='r'
    AND (pg_has_role(actor.oid,t.relowner,'MEMBER')
     OR has_table_privilege(actor.oid,t.oid,'UPDATE,DELETE,TRUNCATE,TRIGGER')
     OR has_any_column_privilege(actor.oid,t.oid,'UPDATE')
     OR (has_any_column_privilege(actor.oid,t.oid,'INSERT') AND NOT
      (($1='dw_m1_trajectory' AND n.nspname='trajectory' AND t.relname='reasoning_history') OR
       ($1='dw_runtime' AND n.nspname IN ('mission','trajectory') AND t.relname NOT IN ('reasoning_history','terrain_history'))))))))";

pub fn qualify_owner(c: &mut Client, role: &str) -> Res<()> {
    let ok: bool = c
        .query_one(QUALIFY_OWNER_SQL, &[&role])
        .map_err(|_| "inference_database_unqualified")?
        .get(0);
    if ok {
        Ok(())
    } else {
        Err("inference_database_unqualified")
    }
}
