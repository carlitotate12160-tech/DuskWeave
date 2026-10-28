//! Immutable semantic call binding; provider wire encoding is not authority.
use super::inference::{Reservation, Scope};
use super::request::BoundCall;
use crate::m1_inference_profile::{self as profiles, Backend};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;
type Res<T> = Result<T, &'static str>;
const VERSION: &str = "dw-inference-intent-v1";

struct CallIdentity {
    generation: u64,
    generation_call: Uuid,
    repair_call: Uuid,
    call: Uuid,
}
impl CallIdentity {
    fn reserved(r: &Reservation, call: Uuid) -> Self {
        Self {
            generation: r.generation,
            generation_call: r.generation_call,
            repair_call: r.repair_call,
            call,
        }
    }
    fn repair(&self) -> Res<bool> {
        if [
            self.generation == 0,
            self.generation_call.is_nil(),
            self.repair_call.is_nil(),
            self.generation_call == self.repair_call,
        ]
        .into_iter()
        .any(|v| v)
        {
            return Err("inference_decision_integrity");
        }
        match self.call {
            id if id == self.generation_call => Ok(false),
            id if id == self.repair_call => Ok(true),
            _ => Err("inference_decision_integrity"),
        }
    }
}

pub fn sdk_pack(pack: &[u8]) -> bool {
    serde_json::from_slice::<Value>(pack)
        .is_ok_and(|p| p["version"] == "m1_context_pack_autonomous_v3")
}
fn parsed_pack(pack: &[u8]) -> Res<Value> {
    let p = super::bounded_json::parse(pack, 8192, 1024).map_err(|_| "request_encode")?;
    let valid = [
        p["version"] == "m1_context_pack_autonomous_v3",
        p["run_profile"] == profiles::RUN_PROFILE,
        p["execution_mode"] == "controlled_fixture",
        p["mission_revision"].as_u64().is_some_and(|n| n > 0),
    ];
    if !valid.into_iter().all(|v| v) {
        return Err("request_encode");
    }
    for field in [
        "engagement_id",
        "campaign_id",
        "decision_id",
        "permission_id",
    ] {
        if uuid(&p[field])?.is_nil() {
            return Err("request_encode");
        }
    }
    Ok(p)
}
fn scope_matches(p: &Value, s: &Scope) -> bool {
    p["engagement_id"] == json!(s.engagement)
        && p["campaign_id"] == json!(s.campaign)
        && p["decision_id"] == json!(s.decision)
        && p["permission_id"] == json!(s.permission)
        && p["mission_revision"] == json!(s.mission_revision)
}
fn canonical(mut value: Value) -> Res<Vec<u8>> {
    value.sort_all_objects();
    let bytes = serde_json::to_vec(&value).map_err(|_| "request_encode")?;
    if bytes.len() > 16384 {
        return Err("request_size");
    }
    Ok(bytes)
}
pub fn digest(pack: &[u8], intent: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(VERSION.as_bytes());
    hash.update([0]);
    for bytes in [pack, intent] {
        hash.update((bytes.len() as u64).to_be_bytes());
        hash.update(bytes);
    }
    hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
}
fn material(pack: &[u8], codes: Option<&[&str]>) -> Res<Value> {
    let p = parsed_pack(pack)?;
    let profile = sdk_profile(&p)?;
    if let Some(codes) = codes {
        if codes.is_empty() {
            return Err("repair_code_ineligible");
        }
        super::request::validate_repair_codes(codes)?;
    }
    let (instructions, schema) = super::request::semantic_material(pack)?;
    let t = profile.timing;
    let limits = json!({"request_bytes":16384,"response_bytes":65536,"context_bytes":8192,
            "configuration_bytes":16384,"connect_time_ms":t.connect_ms,"soft_completion_ms":t.soft_ms,
            "per_call_wall_time_ms":t.hard_ms,"shared_wall_time_ms":t.shared_ms,
            "decision_wall_time_ms":t.decision_ms,"phase_wall_time_ms":t.phase_ms,
            "cancel_grace_ms":t.stop_ms,"accounting_window_ms":t.accounting_ms,
            "output_tokens_per_call":4096,"calls_per_decision":2,"concurrency":1});
    Ok(
        json!({"version":VERSION,"permission_version":"m1_inference_permission_v5",
        "run_version":"m1_autonomous_run_request_v3","engagement_id":p["engagement_id"],
        "campaign_id":p["campaign_id"],"decision_id":p["decision_id"],
        "permission_id":p["permission_id"],"mission_revision":p["mission_revision"],
        "run_profile":profiles::RUN_PROFILE,"execution_mode":"controlled_fixture",
        "protocol":"openai-resp-v1","profile":profile.name,"recipient":profile.connection.recipient,
        "model":profile.settings.model,"adapter_release":"genai-a9504f1-dw1",
        "template_version":"m1-autonomous-inference-v3","schema_version":"m1_inference_proposal_v1",
        "instructions":instructions,"schema":schema,"timing_profile":profiles::TIMING_PROFILE,
        "settings":{"reasoning_effort":"low","max_output_tokens":4096,"stream":false,"store":false,"tools":[]},
        "repair_error_codes":codes.unwrap_or_default(),"limits":limits}),
    )
}
fn sdk_profile(p: &Value) -> Res<profiles::Profile> {
    let profile = profiles::lookup(p["inference_profile"].as_str().ok_or("request_encode")?)
        .map_err(|_| "request_encode")?;
    if profile.backend != Backend::Genai {
        return Err("request_encode");
    }
    Ok(profile)
}
// UUID JSON values always occupy 36 text bytes plus two quotes. Generation is
// bounded by Rust u64's 20 digits (SQL reservations are narrower positive i64).
// Each attached property adds its encoded key, colon, value and one comma to
// the already nonempty material object. No placeholder identity is constructed.
fn attached_identity_bytes() -> Res<usize> {
    [
        ("generation", 20usize),
        ("generation_call_id", 38),
        ("repair_call_id", 38),
        ("call_id", 38),
        ("call_role", 12),
    ]
    .into_iter()
    .try_fold(0usize, |total, (key, value)| {
        let key_bytes = serde_json::to_vec(key).map_err(|_| "request_encode")?.len();
        total
            .checked_add(key_bytes)
            .and_then(|n| n.checked_add(value))
            .and_then(|n| n.checked_add(2))
            .ok_or("request_size")
    })
}
/// Pure pack/intent capacity admission, never a measurement of SDK wire bytes.
pub fn preflight(pack: &[u8], context_cap: usize, configuration_cap: usize) -> Res<usize> {
    if pack.len() > context_cap.min(8192) {
        return Err("request_size");
    }
    let attached = attached_identity_bytes()?;
    [None, Some(&["unexpected_view_request"; 8][..])]
        .into_iter()
        .try_fold(0usize, |largest, codes| {
            let bytes = canonical(material(pack, codes)?)?.len();
            let bound = bytes.checked_add(attached).ok_or("request_size")?;
            if bound > configuration_cap.min(16384) {
                return Err("request_size");
            }
            Ok(largest.max(bound))
        })
}
fn document(pack: &[u8], identity: &CallIdentity, codes: Option<&[&str]>) -> Res<Value> {
    if identity.repair()? != codes.is_some() {
        return Err("repair_code_ineligible");
    }
    let mut intent = material(pack, codes)?;
    let fields = json!({"generation":identity.generation,"generation_call_id":identity.generation_call,
        "repair_call_id":identity.repair_call,"call_id":identity.call,
        "call_role":if codes.is_some() {"repair"} else {"generation"}});
    intent
        .as_object_mut()
        .ok_or("request_encode")?
        .extend(fields.as_object().ok_or("request_encode")?.clone());
    Ok(intent)
}
pub fn bind(
    scope: &Scope,
    reservation: &Reservation,
    call: Uuid,
    pack: &[u8],
    codes: Option<&[&str]>,
) -> Res<BoundCall> {
    if !scope_matches(&parsed_pack(pack)?, scope) {
        return Err("inference_decision_integrity");
    }
    let upper = preflight(pack, 8192, 16384)?;
    let configuration = canonical(document(
        pack,
        &CallIdentity::reserved(reservation, call),
        codes,
    )?)?;
    if configuration.len() > upper {
        return Err("inference_decision_integrity");
    }
    Ok(BoundCall {
        pack: pack.to_vec(),
        digest: digest(pack, &configuration),
        configuration,
        body: Vec::new(),
    })
}
fn uuid(v: &Value) -> Res<Uuid> {
    v.as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or("inference_decision_integrity")
}
fn identity(v: &Value) -> Res<CallIdentity> {
    Ok(CallIdentity {
        generation: v["generation"]
            .as_u64()
            .ok_or("inference_decision_integrity")?,
        generation_call: uuid(&v["generation_call_id"])?,
        repair_call: uuid(&v["repair_call_id"])?,
        call: uuid(&v["call_id"])?,
    })
}
fn codes(v: &Value) -> Res<Vec<&str>> {
    v["repair_error_codes"]
        .as_array()
        .ok_or("inference_decision_integrity")?
        .iter()
        .map(|v| v.as_str().ok_or("inference_decision_integrity"))
        .collect()
}
pub fn verify(bound: &BoundCall) -> Res<Value> {
    if !bound.body.is_empty() || bound.configuration.len() > 16384 {
        return Err("inference_decision_integrity");
    }
    let v: Value =
        serde_json::from_slice(&bound.configuration).map_err(|_| "inference_decision_integrity")?;
    let expected = reconstruct(&bound.pack, &v)?;
    if expected != bound.configuration || digest(&bound.pack, &bound.configuration) != bound.digest
    {
        return Err("inference_decision_integrity");
    }
    Ok(v)
}
fn reconstruct(pack: &[u8], v: &Value) -> Res<Vec<u8>> {
    let id = identity(v)?;
    let codes = codes(v)?;
    canonical(document(
        pack,
        &id,
        id.repair()?.then_some(codes.as_slice()),
    )?)
}
pub fn verify_owned(
    bound: &BoundCall,
    scope: &Scope,
    reservation: &Reservation,
    call: Uuid,
) -> Res<()> {
    let v = verify(bound)?;
    let codes = codes(&v)?;
    let expected = bind(
        scope,
        reservation,
        call,
        &bound.pack,
        (call == reservation.repair_call).then_some(codes.as_slice()),
    )?;
    if expected.configuration != bound.configuration {
        return Err("inference_decision_integrity");
    }
    Ok(())
}
