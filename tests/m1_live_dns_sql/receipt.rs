//! Schema-0030 receipt SQL regressions: qualification field bindings.
use super::fixture::*;
use super::run_support::*;
use duskweave::m1_reasoning::{
    inference::{self, Provider},
    postgres_terrain::TerrainBasis,
    proposal::{ProposalBasis, RecordingPremises},
    request, response,
};
use serde_json::Value;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// The declared LAB manifest only; every authority, dependency and recording
/// check still delegates to the real TerrainBasis owner.
struct LabBasis(TerrainBasis);
impl ProposalBasis for LabBasis {
    fn reference_eligible(&self, r: &str) -> bool {
        self.0.reference_eligible(r)
    }
    fn manifest_eligible(&self, r: &str) -> bool {
        r == "dns_udp"
    }
    fn safe_explanation(&self, s: &str) -> bool {
        self.0.safe_explanation(s)
    }
    fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
        self.0.current_authority_and_premises()
    }
    fn before_provider_release(&mut self) -> Result<(), &'static str> {
        self.0.before_provider_release()
    }
    fn current_authority_for_references(&mut self, r: &[String]) -> Result<(), &'static str> {
        self.0.recording_references(r)
    }
    fn recording_premises(&mut self) -> Result<RecordingPremises, &'static str> {
        self.0.recording_integrity()
    }
}

/// Private in-process provider: invokes the real release closure, derives the
/// synthetic D1 proposal from the bound pack and screens its own serialized
/// envelope through the real response admission.
#[derive(Default)]
struct LiveProvider {
    calls: usize,
    releases: usize,
}
impl Provider for LiveProvider {
    fn prepare(&mut self, call: &request::BoundCall, _: Instant) -> Result<(), &'static str> {
        let pack = std::str::from_utf8(&call.pack).map_err(|_| "pack_utf8")?;
        let conf = std::str::from_utf8(&call.configuration).map_err(|_| "conf_utf8")?;
        let body = std::str::from_utf8(&call.body).map_err(|_| "body_utf8")?;
        request::verify_binding(pack, conf, body, &call.digest, self.calls != 0)
    }
    fn call(
        &mut self,
        call: &request::BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<inference::Received, &'static str> {
        self.calls += 1;
        release()?;
        self.releases += 1;
        let pack: Value = serde_json::from_slice(&call.pack).map_err(|_| "pack_decode")?;
        let raw = serde_json::to_vec(&genai::envelope(&genai::proposal(&pack)))
            .map_err(|_| "envelope_encode")?;
        response::admit_envelope(&raw, raw.len(), Duration::ZERO, basis)
    }
}

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_pg_effect_store_admitted_lifecycle() {
    assert!(worker());
    use duskweave::Fail;
    use duskweave::m1_dns::{DnsAnswer, DnsResult, DnsStatus};
    use duskweave::m1_effect_session::{EffectPhase, EffectStore};
    use duskweave::m1_reasoning::postgres::{Basis, PgDecisionStore};
    use duskweave::m1_run::RunPhase;
    use duskweave::postgres_m1_effect::PgEffectStore;
    use duskweave::postgres_m1_run::PgRunStore;

    // Registration, accepted publication and the sole authority setup insert.
    let (e, c, _operator) = register_live(0x5d03, true);
    let run = Uuid::new_v4();
    let publication = live_publication(e, c, run);
    let qualification = seed_live_qualification(&publication);
    let mut runs = PgRunStore::new(broker());
    assert!(matches!(
        runs.start(
            &successor_request(&publication, run, "m1_autonomous_run_request_v4"),
            RELEASE
        ),
        Ok(true)
    ));
    let record = runs.inspect(run).unwrap();
    let custodian = Renewals::start(run);

    // Real D1 through the restricted broker and separate owner connections.
    let prepared = prepare_decision_one(&mut runs, &record);
    let deadline = Instant::now() + Duration::from_millis(prepared.window_ms as u64);
    let pack = prepared.pack.utf8().as_bytes();
    let mut provider = LiveProvider::default();
    let mut port = broker_port(&record, deadline);
    let mut basis = LabBasis(TerrainBasis {
        mission: Basis {
            client: broker(),
            scope: prepared.scope.clone(),
            references: request::reference_ids(pack).unwrap(),
            deadline,
        },
        dependencies: prepared.pack.terrain_dependencies().to_vec(),
    });
    let receipt = inference::infer(
        &mut port,
        &mut provider,
        &mut basis,
        &mut PgDecisionStore {
            client: client("DW_COGNITION_DATABASE_URL"),
            input_hash: prepared.input_hash.clone(),
        },
        &mut duskweave::postgres_trajectory::PgTrajectory::new(client(
            "DW_TRAJECTORY_DATABASE_URL",
        )),
        inference::InferenceInput {
            scope: &prepared.scope,
            pack,
            event_id: Uuid::new_v4(),
            occurred_at_ms: now_ms() as u64,
            source_mode: "blind".into(),
        },
    );
    assert_eq!((provider.calls, provider.releases), (1, 1));
    assert_eq!(receipt.proposal_disposition, "validated_proposal");
    assert_eq!(receipt.publication, "committed");
    assert_eq!(receipt.history, "completed");
    assert_eq!(
        (receipt.decision, receipt.call_ids.len()),
        (prepared.scope.decision, 2)
    );
    runs.complete_phase(run, RunPhase::DecisionOne).unwrap();
    runs.start_phase(run, RunPhase::Acquisition).unwrap();
    runs.claim_phase(run, RunPhase::Acquisition, RELEASE)
        .unwrap();

    // One closure, one completed call, one decision, one accepted history row.
    let decision = prepared.scope.decision;
    let mut view = admin();
    let provenance: i64 = view
        .query_one("SELECT (SELECT count(*) FROM execution.inference_closures WHERE e=$1 AND c=$2 AND d=$3)+(SELECT count(*) FROM execution.inference_calls WHERE e=$1 AND c=$2 AND d=$3 AND state='completed')+(SELECT count(*) FROM cognition.reasoning_decisions WHERE e=$1 AND c=$2 AND d=$3)+(SELECT count(*) FROM trajectory.reasoning_history WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id=$3 AND status='accepted')", &[&e, &c, &decision])
        .unwrap()
        .get(0);
    assert_eq!(provenance, 4);

    // The store login is a restricted Broker: no power flags, dw_m1_broker
    // membership only (covers every fixture owner role).
    let restricted: bool = broker()
        .query_one("SELECT NOT (r.rolsuper OR r.rolcreaterole OR r.rolcreatedb OR r.rolreplication OR r.rolbypassrls) AND pg_has_role(session_user,'dw_m1_broker','MEMBER') AND NOT EXISTS(SELECT 1 FROM pg_auth_members m JOIN pg_roles g ON g.oid=m.roleid WHERE m.member=r.oid AND g.rolname<>'dw_m1_broker') FROM pg_roles r WHERE r.rolname=session_user", &[])
        .unwrap()
        .get(0);
    assert!(restricted);

    let mut store = PgEffectStore::new(broker());
    let f = |v: &mut postgres::Client| -> i64 {
        v.query_one("SELECT generation FROM execution.session_fences WHERE engagement_id=$1 AND campaign_id=$2 AND operation_id IS NULL AND phase='idle'", &[&e, &c]).unwrap().get(0)
    };
    let q =
        |v: &mut postgres::Client, sql: &str| -> i64 { v.query_one(sql, &[&run]).unwrap().get(0) };
    let ev = |v: &mut postgres::Client| -> Value {
        v.query_one("SELECT coalesce(jsonb_object_agg(phase,n),'{}'::jsonb) FROM (SELECT phase,count(*) n FROM execution.live_effect_events WHERE run_id=$1 GROUP BY phase) t", &[&run]).unwrap().get(0)
    };

    // reserve: one session, one reserved event, one fence advance.
    let session = store.reserve(run).unwrap();
    assert_eq!(session.phase, EffectPhase::Reserved);
    assert!(session.launchable());
    let rq = &session.request;
    assert_eq!((session.run_id, session.result_ref), (run, record.result));
    assert_eq!(
        (rq.engagement_id, rq.campaign_id, rq.decision_id),
        (e, c, decision)
    );
    assert_eq!(
        (rq.attempt, rq.qualification_ref),
        (record.attempt, qualification)
    );
    assert!(rq.session_generation > 0);
    assert_eq!(rq.release_sha256, RELEASE);
    assert_eq!(rq.allowed_names, ["seed.duskweave.test"]);
    assert_eq!(rq.name, "seed.duskweave.test");
    assert_eq!(rq.resolver_ipv4, "192.0.2.53");
    assert_eq!(
        (rq.run_profile.as_str(), rq.execution_mode.as_str()),
        (LAB, "live")
    );
    assert!(rq.deadline_at_ms > now_ms());
    let generation = rq.session_generation as i64;
    assert_eq!(f(&mut view), generation);
    let session_json = serde_json::to_value(&session).unwrap();
    let replay = serde_json::to_value(store.reserve(run).unwrap()).unwrap();
    assert_eq!(replay, session_json);
    let observed = serde_json::to_value(store.inspect(run).unwrap()).unwrap();
    assert_eq!(observed, session_json);
    let n = q(
        &mut view,
        "SELECT count(*) FROM execution.live_effect_sessions WHERE run_id=$1",
    );
    assert_eq!(n, 1);
    assert_eq!(ev(&mut view), serde_json::json!({"reserved": 1}));
    assert_eq!(f(&mut view), generation, "no second fence advance");

    // current_deadline before charge; may_start clamps the attempt clock.
    assert_eq!(
        store.current_deadline(&session),
        Ok(session.attempt_deadline_at_ms)
    );
    let attempt_deadline = store.may_start(&session).unwrap();
    let expected_deadline = q(
        &mut view,
        "SELECT least(s.deadline_at_ms,e.occurred_at_ms+2000) FROM execution.live_effect_sessions s JOIN execution.live_effect_events e USING(run_id) WHERE run_id=$1 AND e.phase='may_start'",
    );
    assert_eq!(attempt_deadline, expected_deadline);
    let charged = store.inspect(run).unwrap();
    assert_eq!(charged.phase, EffectPhase::MayStart);
    assert!(charged.possibly_started);
    assert_eq!(charged.attempt_deadline_at_ms, attempt_deadline);
    let refused = store.may_start(&session);
    assert_eq!(refused, Err(Fail::State("live_effect_release_refused")));
    let after = store.inspect(run).unwrap().attempt_deadline_at_ms;
    assert_eq!(after, attempt_deadline);
    assert_eq!(ev(&mut view)["may_start"], 1);

    // Synthetic completed projection inside the may_start window; no actual
    // DNS observation happened.
    let result = DnsResult {
        schema: "m1_dns_ipc_v1".into(),
        kind: "RESULT".into(),
        attempt: rq.attempt,
        session_generation: rq.session_generation,
        sequence: 3,
        status: DnsStatus::Completed,
        observed_at_ms: now_ms(),
        rcode: 0,
        truncated: false,
        complete: true,
        withheld_aliases: 0,
        answers: vec![DnsAnswer {
            ipv4: "192.0.2.77".into(),
            ttl_seconds: 60,
        }],
        aliases: vec![],
    };
    store.record_result(&charged, &result).unwrap();
    let recorded = store.inspect(run).unwrap();
    assert_eq!(recorded.phase, EffectPhase::ResultRecorded);
    assert_eq!(recorded.result, Some(result.clone()));
    let durable = view
        .query_one("SELECT result,attempt,session_generation,publication_obligation,projection FROM execution.live_dns_results WHERE run_id=$1", &[&run])
        .unwrap();
    assert_eq!(durable.get::<_, Uuid>(0), session.result_ref);
    let (a, g) = (durable.get::<_, Uuid>(1), durable.get::<_, i64>(2));
    assert_eq!((a, g), (rq.attempt, generation));
    assert_eq!(durable.get::<_, String>(3), "terrain.issued_dns.v2");
    assert_eq!(
        durable.get::<_, Value>(4),
        serde_json::to_value(&result).unwrap()
    );

    // Identical replay is idempotent; a shape-valid differing projection hits
    // the SQL conflict branch and leaves stored state unchanged.
    store.record_result(&charged, &result).unwrap();
    let mut conflicting = result.clone();
    conflicting.answers[0].ttl_seconds = 30;
    let conflict = store.record_result(&charged, &conflicting);
    assert_eq!(
        conflict,
        Err(Fail::Unresolved("live_result_commit_unknown"))
    );
    assert_eq!(store.inspect(run).unwrap().result, Some(result.clone()));
    assert_eq!(ev(&mut view)["result_recorded"], 1);

    // Stopped preserves charge/result and refuses further charge; Closed
    // confirms disposal. Repeats duplicate no events and erase no history.
    store.stop(&session).unwrap();
    let stopped = store.inspect(run).unwrap();
    assert_eq!(stopped.phase, EffectPhase::Stopped);
    assert!(stopped.possibly_started);
    assert!(!stopped.disposal_confirmed);
    assert_eq!(stopped.result, Some(result.clone()));
    assert_eq!(
        store.current_deadline(&session),
        Err(Fail::State("live_effect_release_refused"))
    );
    assert_eq!(
        store.may_start(&session),
        Err(Fail::State("live_effect_release_refused"))
    );
    store.close_disposed(&session).unwrap();
    let closed = store.inspect(run).unwrap();
    assert_eq!(closed.phase, EffectPhase::Closed);
    assert!(closed.disposal_confirmed);
    assert_eq!(closed.result, Some(result.clone()));
    store.stop(&session).unwrap();
    store.close_disposed(&session).unwrap();
    let closed_json = serde_json::to_value(&closed).unwrap();
    let again = serde_json::to_value(store.inspect(run).unwrap()).unwrap();
    assert_eq!(again, closed_json);
    assert_eq!(
        ev(&mut view),
        serde_json::json!({"reserved": 1, "may_start": 1, "result_recorded": 1, "stopped": 1, "closed": 1})
    );

    // Historical reserve/inspect return the same closed, nonlaunchable history.
    let history = serde_json::to_value(store.reserve(run).unwrap()).unwrap();
    assert_eq!(history, closed_json);
    assert!(!store.inspect(run).unwrap().launchable());
    let s1 = q(
        &mut view,
        "SELECT count(*) FROM execution.live_effect_sessions WHERE run_id=$1",
    );
    let r1 = q(
        &mut view,
        "SELECT count(*) FROM execution.live_dns_results WHERE run_id=$1",
    );
    let v1 = q(
        &mut view,
        "SELECT count(*) FROM execution.live_effect_events WHERE run_id=$1",
    );
    assert_eq!((s1, r1, v1), (1, 1, 5));
    assert_eq!(f(&mut view), generation);

    // Retire the custodian first, then the run through the restricted port.
    custodian.retire();
    runs.stop(run).unwrap();
    let control = view
        .query_one(
            "SELECT status,lease_until_ms FROM execution.autonomous_control WHERE run_id=$1",
            &[&run],
        )
        .unwrap();
    assert_eq!(control.get::<_, String>(0), "stopped");
    assert_eq!(control.get::<_, i64>(1), 0);
}

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_receipt_binds_qualification_fields() {
    assert!(worker());
    let (e, c, _operator) = register_live(0x5d01, true);
    let (run, _, _) = fabricate_live_run(e, c, false, true);
    let qualified = |run: Uuid| -> bool {
        broker()
            .query_one(
                "SELECT execution.run_summary_without_diagnostics($1)",
                &[&run],
            )
            .unwrap()
            .get::<_, Value>(0)["live_qualified"]
            .as_bool()
            .unwrap()
    };
    assert!(
        qualified(run),
        "matching manifest qualification is live_qualified"
    );
    // Receipt comparison binds release, not merely the qualification ref.
    // Updates scope to this run's own qualification row; other LAB
    // qualifications in the shared fixture must never be touched.
    admin()
        .execute(
            "UPDATE execution.live_qualifications SET release_sha256=$1 WHERE qualification_ref=(SELECT qualification_ref FROM execution.autonomous_runs WHERE run_id=$2)",
            &[&"f".repeat(64), &run],
        )
        .unwrap();
    assert!(!qualified(run), "release mismatch cannot be live_qualified");
    admin()
        .execute(
            "UPDATE execution.live_qualifications SET release_sha256=$1,valid_until_ms=2 WHERE qualification_ref=(SELECT qualification_ref FROM execution.autonomous_runs WHERE run_id=$2)",
            &[&RELEASE, &run],
        )
        .unwrap();
    assert!(
        !qualified(run),
        "expired qualification cannot be live_qualified"
    );
}

#[test]
#[ignore = "runs only inside the dedicated schema-0030 fixture worker"]
fn sql_live_pg_effect_store_rejects_unadmitted_run() {
    assert!(worker());
    use duskweave::Fail;
    use duskweave::m1_effect_session::EffectStore;
    use duskweave::postgres_m1_effect::PgEffectStore;

    let (e, c, _operator) = register_live(0x5d02, true);
    // fabricate_live_run(...,session=false,running=true) bypasses run_start:
    // context.mission is empty, the qualification does not match, and there is
    // no D1/closure/pack provenance. This run must never be admitted.
    let (run, _gen, _result_ref) = fabricate_live_run(e, c, false, true);

    let scoped_state = |run: Uuid| -> Value {
        let mut a = admin();
        let run_rows = |a: &mut postgres::Client, sql: &str| -> Value {
            a.query_one(sql, &[&run]).unwrap().get::<_, Value>(0)
        };
        let scope_rows = |a: &mut postgres::Client, sql: &str| -> Value {
            a.query_one(sql, &[&e, &c]).unwrap().get::<_, Value>(0)
        };
        serde_json::json!({
            "runs": run_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.autonomous_runs t WHERE t.run_id=$1"),
            "control": run_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.autonomous_control t WHERE t.run_id=$1"),
            "fence": scope_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.session_fences t WHERE t.engagement_id=$1 AND t.campaign_id=$2"),
            "sessions": run_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.live_effect_sessions t WHERE t.run_id=$1"),
            "effect_control": run_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.live_effect_control t WHERE t.run_id=$1"),
            "events": run_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.live_effect_events t WHERE t.run_id=$1"),
            "reservations": scope_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.inference_reservations t WHERE t.engagement_id=$1 AND t.campaign_id=$2"),
            "closures": scope_rows(&mut a, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)),'[]'::jsonb) FROM execution.inference_closures t WHERE t.e=$1 AND t.c=$2"),
        })
    };
    let before = scoped_state(run);

    // The restricted broker function itself refuses this malformed request at
    // run_current with the public reason, before any adapter mapping.
    let error = broker()
        .query_one("SELECT execution.run_current($1)", &[&run])
        .unwrap_err();
    assert_eq!(db_message(&error), "run_authority_unavailable");

    // The adapter maps that refusal without inventing a session.
    let mut store = PgEffectStore::new(broker());
    assert_eq!(
        store.reserve(run).map(|_| ()),
        Err(Fail::State("live_effect_unavailable"))
    );

    // An exception alone is not proof: compare scoped run/control, session
    // fence, effect session/control/event and inference reservation/closure
    // rows. No new session, fence advance, charge or effect may be written.
    assert!(
        scoped_state(run) == before,
        "refused calls must not write scoped run/fence/effect or inference state"
    );
}
