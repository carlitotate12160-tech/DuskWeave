//! Target-private registered-run lifecycle support shared by the receipt,
//! successor pack and episode cases; no production API. Owns the lease
//! custodian, the restricted Broker port, the publication-derived typed run
//! request and the installed-function identity witness.
use super::fixture::{admin, broker, client, now_ms};
use duskweave::m1_reasoning::postgres::PgBroker;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// RAII lease renewal custodian for this run only: a separate Broker
/// connection renews at the existing 400 ms test cadence until retired.
pub(crate) struct Renewals {
    stop: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<String>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Renewals {
    pub(crate) fn start(run: Uuid) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let failure = Arc::new(Mutex::new(None));
        let (stopping, failures) = (stop.clone(), failure.clone());
        let worker = std::thread::spawn(move || {
            let mut runs = duskweave::postgres_m1_run::PgRunStore::new(broker());
            while !stopping.load(Ordering::SeqCst) {
                if let Err(error) = runs.renew(run) {
                    *failures.lock().unwrap() = Some(format!("{error:?}"));
                    return;
                }
                std::thread::sleep(Duration::from_millis(400));
            }
        });
        Self {
            stop,
            failure,
            worker: Some(worker),
        }
    }
    pub(crate) fn retire(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            worker.join().expect("lease custodian join");
        }
        let failure = self.failure.lock().unwrap().take();
        assert!(failure.is_none(), "lease renewal failed: {failure:?}");
    }
}
impl Drop for Renewals {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            assert!(worker.join().is_ok() || std::thread::panicking());
        }
    }
}

/// Installed assert_successor_pack body identity. The selected 0030 function
/// must be retained verbatim across every producer/consumer assertion and
/// CLI action: no refresh or hand-applied replacement inside a case.
pub(crate) fn successor_pack_identity() -> String {
    admin()
        .query_one(
            "SELECT md5(prosrc) FROM pg_proc WHERE oid=\
             'execution.assert_successor_pack(jsonb,jsonb,jsonb)'::regprocedure",
            &[],
        )
        .unwrap()
        .get(0)
}

/// The restricted Broker port: same owner DSNs as the real decision path.
/// The caller supplies the absolute deadline measured at preparation time.
pub(crate) fn broker_port(record: &duskweave::m1_run::RunRecord, deadline: Instant) -> PgBroker {
    PgBroker {
        client: broker(),
        mission: duskweave::postgres_mission::PgMissionStore::new(client("DW_TEST_DATABASE_URL")),
        reservations: duskweave::postgres_inference_budget::PgInferenceReservationStore::new(
            broker(),
        ),
        request: record.request.context.mission.clone(),
        deadline,
    }
}

/// Typed successor run request derived entirely from the accepted
/// publication; permission carries run_profile/execution_mode/profile/binding.
pub(crate) fn successor_request(
    publication: &Value,
    run: Uuid,
    version: &str,
) -> duskweave::m1_run::RunRequest {
    let fields = &publication["fields"];
    let permission = &fields["m1_inference_permission"];
    let asset = fields["included_assets"][0].as_str().unwrap();
    let mut raw = json!({
        "version":version,"run_id":run,
        "run_profile":permission["run_profile"],"execution_mode":permission["execution_mode"],
        "deadline_at_ms":now_ms()+300000,"inference_profile":permission["profile"],
        "context":{"version":"m1_context_prepare_request_v1","mission":{
            "version":"m1_inference_reservation_request_v1",
            "engagement_id":publication["engagement_id"],
            "campaign_id":publication["campaign_id"],
            "operator_ref":fields["operator_ref"],"expected_mission_revision":1,
            "permission_id":permission["permission_id"],"goal_ref":fields["goal_ref"],
            "exercise_mode":fields["exercise_mode"]},
            "candidates":[{"asset_ref":asset,"priority":"high"}]},
        "seed_names":[{"asset_ref":asset,"name":"seed.duskweave.test"}]});
    if version == "m1_autonomous_run_request_v4" {
        raw["live_binding"] = permission["live_binding"].clone();
    }
    duskweave::m1_run::RunRequest::parse(&serde_json::to_vec(&raw).unwrap())
        .expect("typed successor run request")
}
