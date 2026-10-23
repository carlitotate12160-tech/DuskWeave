//! Explicit local inference double. Its second proposal depends on the supplied qualified feedback.
use super::inference::{Provider, Received, Usage};
use super::proposal::{self, ProposalBasis};
use super::request::BoundCall;
use super::transport_control::{CallClock, Permit, ReportedGuard};
use serde_json::{Value, json};
use std::time::Instant;

#[derive(Default)]
pub struct DeterministicProvider;
impl Provider for DeterministicProvider {
    fn prepare(&mut self, call: &BoundCall, deadline: Instant) -> Result<(), &'static str> {
        let p: Value = serde_json::from_slice(&call.pack).map_err(|_| "request_encode")?;
        let valid = [
            p["run_profile"] == "autonomous-dns-fixture-v1",
            p["execution_mode"] == "controlled_fixture",
            Instant::now() < deadline,
        ];
        valid
            .into_iter()
            .all(|v| v)
            .then_some(())
            .ok_or("inference_fixture_endpoint_refused")
    }
    fn call(
        &mut self,
        call: &BoundCall,
        deadline: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        self.prepare(call, deadline)?;
        release()?;
        let pack: Value = serde_json::from_slice(&call.pack).map_err(|_| "request_encode")?;
        let raw = proposal_for(&pack)?;
        let bytes = serde_json::to_vec(&raw).map_err(|_| "request_encode")?;
        Ok(Received {
            admission: proposal::admit(&bytes, basis),
            accounting_confirmed: false,
            usage: Usage {
                input_tokens: None,
                output_tokens: None,
                reasoning_tokens: None,
                response_bytes: bytes.len(),
                wall_time_ms: 0,
                requested_model: "deterministic-autonomous-v1",
                reported_model: Some("deterministic-autonomous-v1".into()),
            },
        })
    }
}
fn proposal_for(pack: &Value) -> Result<Value, &'static str> {
    let (references, intent, summary) = if pack["predecessor"]["availability"] == "committed" {
        feedback(pack)?
    } else {
        let seed = pack["candidates"][0]["reference"]
            .as_str()
            .ok_or("unsupported_premise")?;
        (
            json!([seed]),
            json!({"category":"observe_more","references":[seed],"manifest_reference":"dns_udp","view_request":null}),
            "The admitted seed needs a bounded fixture routing observation.",
        )
    };
    Ok(
        json!({"version":"m1_inference_proposal_v1","hypothesis":summary,
        "considered_alternative":"The routing premise may be unavailable.","references":references,
        "expected_observation":"An owner-qualified routing outcome with explicit fixture provenance.",
        "falsifying_observation":"Missing, expired or conflicted outcome.","uncertainty":"Fixture plumbing only; no live target fact or cognitive qualification.",
        "rationale":summary,"next_intent":intent}),
    )
}
fn feedback(pack: &Value) -> Result<(Value, Value, &'static str), &'static str> {
    let claim = pack["references"]
        .as_array()
        .ok_or("request_encode")?
        .iter()
        .find(|r| {
            r["owner"] == "cyber_terrain"
                && r["source"] == "fixture_dns_acquisition"
                && r["status"] == "observed"
        });
    let id = claim
        .and_then(|r| r["id"].as_str())
        .ok_or("reference_ineligible")?;
    Ok((
        json!(["previous_decision", id]),
        json!({"category":"retain","references":[id],"manifest_reference":null}),
        "The newly qualified fixture outcome supports retaining the routing hypothesis; live applicability remains unknown.",
    ))
}

/// Explicit protocol double: request bytes, envelope and normal screening are real.
pub struct ProtocolFixture {
    profile: crate::m1_inference_profile::Profile,
    prepared: Option<String>,
    control: Option<FixtureControl>,
}
struct FixtureControl {
    permit: Permit,
    reporter: Option<std::sync::mpsc::SyncSender<ReportedGuard>>,
    clock: Option<CallClock>,
}
impl FixtureControl {
    fn prepare(&mut self, deadline: Instant) -> Result<(), &'static str> {
        self.clock = Some(CallClock::arm(
            self.permit.clone(),
            deadline,
            self.reporter.clone(),
        )?);
        Ok(())
    }
    fn take_clock(&mut self, deadline: Instant) -> Result<CallClock, &'static str> {
        let clock = self.clock.take().ok_or("inference_guard_missing")?;
        clock.remaining(&self.permit, deadline)?;
        Ok(clock)
    }
}
impl ProtocolFixture {
    pub fn new(profile: crate::m1_inference_profile::Profile) -> Result<Self, &'static str> {
        if profile.connection.credential_environment.is_some() {
            return Err("inference_fixture_endpoint_refused");
        }
        Ok(Self {
            profile,
            prepared: None,
            control: None,
        })
    }
    pub fn controlled(
        profile: crate::m1_inference_profile::Profile,
        permit: Permit,
        reporter: Option<std::sync::mpsc::SyncSender<ReportedGuard>>,
    ) -> Result<Self, &'static str> {
        let mut provider = Self::new(profile)?;
        provider.control = Some(FixtureControl {
            permit,
            reporter,
            clock: None,
        });
        Ok(provider)
    }
    fn begin_call(
        &mut self,
        call: &BoundCall,
        deadline: Instant,
    ) -> Result<Option<CallClock>, &'static str> {
        if self.prepared.take().as_deref() != Some(call.digest.as_str())
            || Instant::now() >= deadline
        {
            return Err("inference_deadline");
        }
        self.control
            .as_mut()
            .map(|c| c.take_clock(deadline))
            .transpose()
    }
    fn finish_call(
        &self,
        clock: &Option<CallClock>,
        deadline: Instant,
    ) -> Result<(), &'static str> {
        match (&self.control, clock) {
            (Some(control), Some(clock)) => clock.remaining(&control.permit, deadline).map(|_| ()),
            (None, None) => Ok(()),
            _ => Err("inference_guard_missing"),
        }
    }
    fn request_pack(&self, call: &BoundCall) -> Result<Value, &'static str> {
        let b: Value = serde_json::from_slice(&call.body).map_err(|_| "request_encode")?;
        let (text, repair) = fixture_input(self.profile.protocol, &b);
        verify_fixture_call(call, repair)?;
        let p: Value =
            serde_json::from_str(text.ok_or("request_encode")?).map_err(|_| "request_encode")?;
        if p["inference_profile"] != self.profile.name {
            return Err("inference_fixture_endpoint_refused");
        }
        Ok(p)
    }
    fn receive(
        &self,
        pack: &Value,
        basis: &mut dyn ProposalBasis,
        elapsed: std::time::Duration,
    ) -> Result<Received, &'static str> {
        use crate::m1_inference_profile::Protocol;
        let raw = fixture_envelope(self.profile.protocol, pack)?;
        let mut received = match self.profile.protocol {
            Protocol::Responses => {
                super::response::admit_envelope(&raw, raw.len(), elapsed, basis)?
            }
            Protocol::ChatCompletions => {
                super::chat_completions::admit_envelope(&raw, elapsed, basis)?
            }
        };
        received.usage.requested_model = self.profile.settings.model;
        Ok(received)
    }
}
fn fixture_input(
    protocol: crate::m1_inference_profile::Protocol,
    b: &Value,
) -> (Option<&str>, bool) {
    use crate::m1_inference_profile::Protocol;
    match protocol {
        Protocol::Responses => (
            b["input"]
                .as_str()
                .or_else(|| b["input"][0]["content"].as_str()),
            b["input"].is_array(),
        ),
        Protocol::ChatCompletions => (
            b["messages"][1]["content"].as_str(),
            b["messages"].as_array().is_some_and(|v| v.len() == 3),
        ),
    }
}
fn fixture_envelope(
    protocol: crate::m1_inference_profile::Protocol,
    pack: &Value,
) -> Result<Vec<u8>, &'static str> {
    use crate::m1_inference_profile::Protocol;
    let proposal = proposal_for(pack)?.to_string();
    let envelope = match protocol {
        Protocol::Responses => {
            json!({"model":"protocol-fixture-v1","status":"completed","output":[{
            "type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":proposal}]}]})
        }
        Protocol::ChatCompletions => json!({"model":"protocol-fixture-v1","choices":[{
            "index":0,"finish_reason":"stop","message":{"role":"assistant","content":proposal}}]}),
    };
    serde_json::to_vec(&envelope).map_err(|_| "provider_protocol")
}
fn verify_fixture_call(call: &BoundCall, repair: bool) -> Result<(), &'static str> {
    super::request::verify_binding(
        std::str::from_utf8(&call.pack).map_err(|_| "request_encode")?,
        std::str::from_utf8(&call.configuration).map_err(|_| "request_encode")?,
        std::str::from_utf8(&call.body).map_err(|_| "request_encode")?,
        &call.digest,
        repair,
    )
}
impl Provider for ProtocolFixture {
    fn prepare(&mut self, call: &BoundCall, deadline: Instant) -> Result<(), &'static str> {
        if Instant::now() >= deadline || self.prepared.is_some() {
            return Err("inference_deadline");
        }
        self.request_pack(call)?;
        if let Some(control) = &mut self.control {
            control.prepare(deadline)?;
        }
        self.prepared = Some(call.digest.clone());
        Ok(())
    }
    fn call(
        &mut self,
        call: &BoundCall,
        deadline: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<Received, &'static str> {
        let clock = self.begin_call(call, deadline)?;
        let start = clock.as_ref().map_or_else(Instant::now, |c| c.origin);
        let pack = self.request_pack(call)?;
        self.finish_call(&clock, deadline)?;
        release()?;
        let received = self.receive(&pack, basis, start.elapsed())?;
        self.finish_call(&clock, deadline)?;
        Ok(received)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    struct BlockedBasis;
    impl ProposalBasis for BlockedBasis {
        fn reference_eligible(&self, r: &str) -> bool {
            r == "asset_0"
        }
        fn manifest_eligible(&self, r: &str) -> bool {
            r == "dns_udp"
        }
        fn safe_explanation(&self, _: &str) -> bool {
            true
        }
        fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
            println!("screening_blocked");
            loop {
                std::thread::park();
            }
        }
    }
    #[test]
    fn controlled_fixture_worker() {
        let Ok(name) = std::env::var("DW_CONTROLLED_PROTOCOL_WORKER") else {
            return;
        };
        let end = Instant::now() + Duration::from_millis(124000);
        let permit = Permit::new(end);
        assert!(permit.publish(Instant::now(), end));
        let (reporter, full_queue) = std::sync::mpsc::sync_channel(1);
        reporter.try_send(ReportedGuard::CommandWatchdog).unwrap();
        let _full_queue = full_queue;
        let profile = crate::m1_inference_profile::lookup(&name).unwrap();
        let mut provider = ProtocolFixture::controlled(profile, permit, Some(reporter)).unwrap();
        let pack=json!({"version":"m1_context_pack_autonomous_v2","run_profile":"autonomous-dns-fixture-v2",
            "execution_mode":"controlled_fixture","inference_profile":name,"references":[{"id":"asset_0"}],
            "candidates":[{"reference":"asset_0"}],"manifests":[{"reference":"dns_udp"}]}).to_string();
        let b = super::super::request::bind(pack.as_bytes(), None).unwrap();
        provider.prepare(&b, end).unwrap();
        let _ = provider.call(&b, end, &mut BlockedBasis, &mut || {
            println!("fixture_released");
            Ok(())
        });
        panic!("blocked screening returned without the independent guard");
    }
    #[test]
    fn both_controlled_protocols_terminate_blocked_screening_when_lease_and_reporter_are_lost() {
        use std::process::{Command, Stdio};
        for name in ["fixture-responses-v1", "fixture-chat-completions-v1"] {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command.env_clear();
            for key in ["SystemRoot", "LLVM_PROFILE_FILE"] {
                if let Some(value) = std::env::var_os(key) {
                    command.env(key, value);
                }
            }
            let started = Instant::now();
            let mut child = command
                .args([
                    "--exact",
                    "m1_reasoning::fixture::tests::controlled_fixture_worker",
                    "--nocapture",
                ])
                .env("DW_CONTROLLED_PROTOCOL_WORKER", name)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            while child.try_wait().unwrap().is_none() {
                if started.elapsed() > Duration::from_secs(5) {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("custodian cleanup is not product termination");
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let output = child.wait_with_output().unwrap();
            assert_eq!(output.status.code(), Some(124));
            assert!(String::from_utf8_lossy(&output.stdout).contains("screening_blocked"));
            assert!(started.elapsed() < Duration::from_millis(2300));
            println!(
                "controlled {name} stopped blocked screening after {}ms",
                started.elapsed().as_millis()
            );
        }
    }
}
