//! Explicit local inference double. Its second proposal depends on the supplied qualified feedback.
use super::inference::{Provider, Received, Usage};
use super::proposal::{self, ProposalBasis};
use super::request::BoundCall;
use super::transport_control::{CallClock, Permit, ReportedGuard, TransportGuard};
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
            json!({"category":"observe_more","references":["mission",seed],"manifest_reference":"dns_udp","view_request":null}),
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
    admission_guard: Option<TransportGuard>,
}
impl FixtureControl {
    fn prepare(&mut self, deadline: Instant) -> Result<(), &'static str> {
        self.clock = Some(CallClock::arm(
            self.permit.clone(),
            deadline,
            self.reporter.clone(),
        )?);
        self.admission_guard = None;
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
        if profile.backend != crate::m1_inference_profile::Backend::Legacy
            || profile.connection.credential_environment.is_some()
        {
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
            admission_guard: None,
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
        raw: &[u8],
        basis: &mut dyn ProposalBasis,
        elapsed: std::time::Duration,
    ) -> Result<Received, &'static str> {
        use crate::m1_inference_profile::Protocol;
        let mut received = match self.profile.protocol {
            Protocol::Responses => super::response::admit_envelope(raw, raw.len(), elapsed, basis)?,
            Protocol::ChatCompletions => {
                super::chat_completions::admit_envelope(raw, elapsed, basis)?
            }
        };
        received.usage.requested_model = self.profile.settings.model;
        Ok(received)
    }
    /// Transfer a completed wire envelope to permit-bounded screening and owner work.
    fn admit_raw(
        &mut self,
        raw: &[u8],
        clock: Option<CallClock>,
        deadline: Instant,
        basis: &mut dyn ProposalBasis,
        elapsed: std::time::Duration,
    ) -> Result<Received, &'static str> {
        if let Some(control) = &mut self.control {
            control.admission_guard = Some(TransportGuard::arm_admission(
                control.permit.clone(),
                deadline,
                control.reporter.clone(),
            )?);
        }
        drop(clock);
        let received = self.receive(raw, basis, elapsed)?;
        if Instant::now() >= deadline || self.control.as_ref().is_some_and(|c| !c.permit.current())
        {
            return Err("current_authority_unavailable");
        }
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
        let raw = fixture_envelope(self.profile.protocol, &pack)?;
        self.finish_call(&clock, deadline)?;
        self.admit_raw(&raw, clock, deadline, basis, start.elapsed())
    }
}

#[cfg(test)]
#[path = "../../tests/support/m1_reasoning_fixture.rs"]
mod tests;
