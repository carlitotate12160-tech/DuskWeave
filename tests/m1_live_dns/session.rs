//! Faults at the real Broker controller's ports; these are not physical host witnesses.
use duskweave::m1_dns::{DnsBoundary, DnsRequest, DnsResult};
use duskweave::m1_effect_control::acquire;
use duskweave::m1_effect_session::{EffectPhase, EffectSession, EffectStore};
use duskweave::{Fail, Res};
use serde_json::json;
use std::cell::RefCell;
use std::rc::Rc;
use uuid::Uuid;

type Log = Rc<RefCell<Vec<&'static str>>>;
struct Store {
    session: EffectSession,
    log: Log,
    fault: Option<&'static str>,
}
struct Boundary {
    available_after: Option<std::time::Instant>,
    result: DnsResult,
    log: Log,
    fault: Option<&'static str>,
}

fn gate(log: &Log, fault: Option<&str>, at: &'static str) -> Res<()> {
    log.borrow_mut().push(at);
    if fault == Some(at) {
        return Err(Fail::Unresolved("injected_port_failure"));
    }
    Ok(())
}

impl EffectStore for Store {
    fn reserve(&mut self, _: Uuid) -> Res<EffectSession> {
        gate(&self.log, self.fault, "reserve")?;
        Ok(self.session.clone())
    }
    fn inspect(&mut self, _: Uuid) -> Res<EffectSession> {
        gate(&self.log, self.fault, "inspect")?;
        Ok(self.session.clone())
    }
    fn current_deadline(&mut self, _: &EffectSession) -> Res<i64> {
        gate(&self.log, self.fault, "current")?;
        Ok(self.session.attempt_deadline_at_ms)
    }
    fn may_start(&mut self, _: &EffectSession) -> Res<i64> {
        gate(&self.log, self.fault, "may_start")?;
        self.session.phase = EffectPhase::MayStart;
        self.session.possibly_started = true;
        Ok(self.session.attempt_deadline_at_ms)
    }
    fn record_result(&mut self, _: &EffectSession, result: &DnsResult) -> Res<()> {
        gate(&self.log, self.fault, "record")?;
        self.session.result = Some(result.clone());
        self.session.phase = EffectPhase::ResultRecorded;
        // A durable result with a lost acknowledgment stays known; it is never replayed.
        gate(&self.log, self.fault, "after_result_commit")
    }
    fn stop(&mut self, _: &EffectSession) -> Res<()> {
        gate(&self.log, self.fault, "sql_stop")?;
        self.session.phase = EffectPhase::Stopped;
        Ok(())
    }
    fn close_disposed(&mut self, _: &EffectSession) -> Res<()> {
        gate(&self.log, self.fault, "sql_close")?;
        self.session.phase = EffectPhase::Closed;
        self.session.disposal_confirmed = true;
        Ok(())
    }
}

impl DnsBoundary for Boundary {
    fn ready(&mut self, _: &DnsRequest) -> Res<()> {
        gate(&self.log, self.fault, "ready")
    }
    fn start(&mut self, _: &DnsRequest, _: i64) -> Res<()> {
        gate(&self.log, self.fault, "start")
    }
    fn result(&mut self) -> Res<Option<DnsResult>> {
        gate(&self.log, self.fault, "result")?;
        if self
            .available_after
            .is_some_and(|end| std::time::Instant::now() < end)
        {
            return Ok(None);
        }
        Ok(Some(self.result.clone()))
    }
    fn renew(&mut self, _: &DnsRequest, _: i64) -> Res<()> {
        gate(&self.log, self.fault, "renew")
    }
    fn acknowledge(&mut self, _: &DnsRequest) -> Res<bool> {
        gate(&self.log, self.fault, "ack")?;
        Ok(true)
    }
    fn stop(&mut self) -> Res<bool> {
        gate(&self.log, self.fault, "host_stop")?;
        Ok(true)
    }
}

fn fixture(fault: Option<&'static str>) -> (Store, Boundary, Log) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let request:DnsRequest=serde_json::from_value(json!({"schema":"m1_dns_ipc_v1","kind":"REQUEST",
  "attempt":Uuid::from_u128(4),"session_generation":1,"sequence":0,"engagement_id":Uuid::from_u128(1),
  "campaign_id":Uuid::from_u128(2),"decision_id":Uuid::from_u128(3),"profile":"dns-routing-v2",
  "execution_mode":"live","run_profile":"autonomous-dns-genai-lab-v1","release_sha256":"a".repeat(64),
  "qualification_ref":Uuid::from_u128(7),"name":"seed.duskweave.test","resolver_ipv4":"192.0.2.53",
  "transport":"udp","query_id":1,"allowed_names":["seed.duskweave.test"],"deadline_at_ms":2000,
  "max_duration_ms":2000})).unwrap();
    let result:DnsResult=serde_json::from_value(json!({"schema":"m1_dns_ipc_v1","kind":"RESULT",
  "attempt":request.attempt,"session_generation":1,"sequence":3,"status":"completed","observed_at_ms":1000,
  "rcode":0,"truncated":false,"complete":true,"withheld_aliases":0,
  "answers":[{"ipv4":"198.51.100.7","ttl_seconds":30}],"aliases":[]})).unwrap();
    let session = EffectSession {
        run_id: Uuid::from_u128(5),
        request,
        result_ref: Uuid::from_u128(6),
        phase: EffectPhase::Reserved,
        possibly_started: false,
        attempt_deadline_at_ms: 2000,
        result: None,
        disposal_confirmed: false,
    };
    session.validate_history().unwrap();
    (
        Store {
            session,
            log: log.clone(),
            fault,
        },
        Boundary {
            available_after: None,
            result,
            log: log.clone(),
            fault,
        },
        log,
    )
}

#[test]
fn controller_requires_fresh_check_and_durable_result_before_ack() {
    let (mut store, mut boundary, log) = fixture(None);
    let run = store.session.run_id;
    acquire(&mut store, &mut boundary, run).unwrap();
    assert_eq!(
        &*log.borrow(),
        &[
            "reserve",
            "ready",
            "may_start",
            "inspect",
            "current",
            "start",
            "result",
            "record",
            "after_result_commit",
            "ack",
            "sql_close"
        ]
    );
    assert!(
        store.session.possibly_started
            && store.session.disposal_confirmed
            && store.session.result.is_some()
    );
    assert_eq!(store.session.phase, EffectPhase::Closed);
}

#[test]
fn failures_stop_the_host_before_sql_and_never_acknowledge_an_uncommitted_result() {
    for fault in [
        "ready",
        "may_start",
        "inspect",
        "current",
        "start",
        "result",
        "record",
        "after_result_commit",
        "ack",
    ] {
        let (mut store, mut boundary, log) = fixture(Some(fault));
        let run = store.session.run_id;
        assert!(acquire(&mut store, &mut boundary, run).is_err(), "{fault}");
        let events = log.borrow();
        let stop = events.iter().position(|v| *v == "host_stop").unwrap();
        let close = events.iter().position(|v| *v == "sql_close").unwrap();
        assert!(stop < close, "host stop precedes bookkeeping: {fault}");
        assert_eq!(
            events.iter().filter(|v| **v == "start").count(),
            usize::from(!["ready", "may_start", "inspect", "current"].contains(&fault))
        );
        if ["record", "after_result_commit"].contains(&fault) {
            assert!(!events.contains(&"ack"));
        }
        if fault == "after_result_commit" {
            assert!(store.session.result.is_some() && store.session.possibly_started);
        }
    }
}

#[test]
fn an_existing_charged_attempt_cannot_reenter_ready_or_start() {
    let (mut store, mut boundary, log) = fixture(None);
    let run = store.session.run_id;
    store.session.phase = EffectPhase::MayStart;
    store.session.possibly_started = true;
    assert!(acquire(&mut store, &mut boundary, run).is_err());
    assert_eq!(&*log.borrow(), &["reserve"]);
    assert!(store.session.possibly_started && store.session.result.is_none());
}

#[test]
fn duplicate_projection_values_are_rejected_before_result_commit_or_ack() {
    let (mut store, mut boundary, log) = fixture(None);
    let run = store.session.run_id;
    boundary
        .result
        .answers
        .push(boundary.result.answers[0].clone());
    assert!(acquire(&mut store, &mut boundary, run).is_err());
    assert!(!log.borrow().contains(&"record") && !log.borrow().contains(&"ack"));
    assert!(store.session.possibly_started && store.session.result.is_none());
}

#[test]
#[ignore = "requires the packet-owned schema-0030 disposable database and restricted logins"]
fn restricted_sql_roles_cannot_forge_live_records_or_refund_counters() {
    use postgres::{Client, NoTls, error::SqlState};
    for variable in [
        "DW_TEST_DATABASE_URL",
        "DW_INFERENCE_BROKER_DATABASE_URL",
        "DW_COGNITION_DATABASE_URL",
        "DW_TRAJECTORY_DATABASE_URL",
        "DW_TERRAIN_DATABASE_URL",
        "DW_STOP_WRITER_DATABASE_URL",
    ] {
        let dsn = std::env::var(variable).expect("restricted fixture configuration required");
        let mut client = Client::connect(&dsn, NoTls)
            .unwrap_or_else(|_| panic!("restricted fixture connection failed: {variable}"));
        let qualified: bool = client.query_one(
            "SELECT current_user=session_user AND NOT (rolsuper OR rolbypassrls OR rolcreaterole OR rolcreatedb OR rolreplication) FROM pg_roles WHERE rolname=current_user",
            &[],
        ).expect("restricted principal inspection").get(0);
        assert!(qualified, "actual restricted login required: {variable}");
        for statement in [
            "INSERT INTO execution.live_qualifications DEFAULT VALUES",
            "INSERT INTO execution.live_effect_sessions DEFAULT VALUES",
            "INSERT INTO execution.live_dns_results DEFAULT VALUES",
            "UPDATE execution.live_effect_control SET possibly_started=false",
            "DELETE FROM execution.live_effect_events",
            "INSERT INTO execution.private_live_writer_context DEFAULT VALUES",
        ] {
            let error = client
                .batch_execute(statement)
                .expect_err("runtime roles cannot forge or erase live accounting");
            assert_eq!(
                error.code(),
                Some(&SqlState::INSUFFICIENT_PRIVILEGE),
                "owner-bound permission denial required: {variable}"
            );
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires the owned schema-0030 b and protected fixture administrator"]
fn sql_safe_projection_admits_canonical_ipv4_and_rejects_duplicates() {
    let mut host = super::host::Host::new();
    let (store, boundary, _) = fixture(None);
    let request = serde_json::to_value(&store.session.request).unwrap();
    let mut result = serde_json::to_value(&boundary.result).unwrap();
    let admitted: bool = host
        .admin
        .query_one(
            "SELECT execution.private_live_result_shape($1,$2,$3,$4)",
            &[&result, &request, &2000i64, &900i64],
        )
        .expect("safe projection shape contract")
        .get(0);
    assert!(
        admitted,
        "canonical IPv4 from the trusted exporter must enter the durable result"
    );
    let answer = result["answers"][0].clone();
    result["answers"].as_array_mut().unwrap().push(answer);
    let admitted: bool = host
        .admin
        .query_one(
            "SELECT execution.private_live_result_shape($1,$2,$3,$4)",
            &[&result, &request, &2000i64, &900i64],
        )
        .unwrap()
        .get(0);
    assert!(!admitted, "duplicate canonical addresses remain refused");
}

#[test]
fn terminal_timeout_can_arrive_in_the_original_post_attempt_handling_window() {
    use std::time::{Duration, Instant};
    let (mut store, mut boundary, log) = fixture(None);
    boundary.result = serde_json::from_value(json!({
        "schema":"m1_dns_ipc_v1","kind":"RESULT",
        "attempt":store.session.request.attempt,"session_generation":1,"sequence":3,
        "status":"timeout","observed_at_ms":0,"rcode":0,"truncated":false,
        "complete":false,"withheld_aliases":0,"answers":[],"aliases":[]
    }))
    .unwrap();
    boundary.available_after = Some(Instant::now() + Duration::from_millis(2200));
    let run = store.session.run_id;
    acquire(&mut store, &mut boundary, run)
        .expect("terminal result uses handling time without extending the wire attempt");
    assert!(store.session.result.is_some() && store.session.disposal_confirmed);
    assert_eq!(log.borrow().iter().filter(|v| **v == "renew").count(), 3);
    assert_eq!(log.borrow().iter().filter(|v| **v == "start").count(), 1);
}
