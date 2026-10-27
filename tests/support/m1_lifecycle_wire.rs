//! Encoded provider doubles and restricted wire admission qualification.
use crate::{
    lifecycle::{Prepared, broker},
    native,
};
use duskweave::m1_reasoning::{
    inference::{self, Provider},
    proposal::ProposalBasis,
    request,
};
use duskweave::m1_run::RunRecord;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Private encoded-response construction; all output still crosses the selected real codec.
pub struct WireFixture {
    pub hypothesis: String,
    pub fault: &'static str,
    pub entries: usize,
}
impl Provider for WireFixture {
    fn prepare(&mut self, b: &request::BoundCall, _: Instant) -> Result<(), &'static str> {
        request::verify_binding(
            std::str::from_utf8(&b.pack).unwrap(),
            std::str::from_utf8(&b.configuration).unwrap(),
            std::str::from_utf8(&b.body).unwrap(),
            &b.digest,
            self.entries != 0,
        )
    }
    fn call(
        &mut self,
        b: &request::BoundCall,
        _: Instant,
        basis: &mut dyn ProposalBasis,
        release: &mut dyn FnMut() -> Result<(), &'static str>,
    ) -> Result<inference::Received, &'static str> {
        release()?;
        self.entries += 1;
        if self.fault == "provider" {
            return Err("provider_transport_unknown");
        }
        let proposal = if self.fault == "repair" && self.entries == 1 {
            json!({"incomplete":"shape"})
        } else {
            json!({"version":"m1_inference_proposal_v1","hypothesis":self.hypothesis,
                "considered_alternative":"Routing remains unavailable.","references":["asset_0"],
                "expected_observation":"Fixture routing metadata.","falsifying_observation":"No qualified current premise.",
                "uncertainty":"Synthetic evidence only.","rationale":"A proposal grants no dispatch.",
                "next_intent":{"category":"observe_more","references":["asset_0"],"manifest_reference":"dns_udp","view_request":null}})
        };
        let chat = serde_json::from_slice::<Value>(&b.configuration).unwrap()["protocol"]
            == "chat-completions-v1";
        let envelope = if chat {
            json!({"choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":proposal.to_string()}}]})
        } else {
            json!({"status":"completed","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":proposal.to_string()}]}]})
        };
        let raw = serde_json::to_vec(&envelope).unwrap();
        let mut received = if chat {
            duskweave::m1_reasoning::chat_completions::admit_envelope(&raw, Duration::ZERO, basis)?
        } else {
            duskweave::m1_reasoning::response::admit_envelope(
                &raw,
                raw.len(),
                Duration::ZERO,
                basis,
            )?
        };
        received.usage.requested_model = "protocol-fixture-v1";
        Ok(received)
    }
}

pub fn eight_seeds(raw: &mut Value) {
    let rule = raw["m1_permission"]["contact_rules"][0].clone();
    let assets: Vec<_> = (0..8)
        .map(|i| Uuid::from_u128(if i == 0 { 0x21 } else { 0x100 + i }))
        .collect();
    raw["included_assets"] = json!(assets);
    raw["m1_permission"]["contact_rules"] = json!(
        assets
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let mut rule = rule.clone();
                rule["asset_ref"] = json!(id);
                rule["priority"] = json!(i + 1);
                if i > 0 {
                    rule["rule"] = json!({"exact":format!("seed{i}.example.invalid")});
                }
                rule
            })
            .collect::<Vec<_>>()
    );
}

pub fn wire_case(name: &str, pack: &str) -> Value {
    let generation = request::bind(pack.as_bytes(), None).unwrap();
    let repair = request::bind(pack.as_bytes(), Some(&["unexpected_view_request"; 8])).unwrap();
    for (bound, is_repair) in [(&generation, false), (&repair, true)] {
        request::verify_binding(
            pack,
            std::str::from_utf8(&bound.configuration).unwrap(),
            std::str::from_utf8(&bound.body).unwrap(),
            &bound.digest,
            is_repair,
        )
        .unwrap();
    }
    println!(
        "wire {name}: P={} C={} generation={} repair={} margins={}/{}/{}",
        pack.len(),
        generation.configuration.len(),
        generation.body.len(),
        repair.body.len(),
        8192 - pack.len(),
        16384 - generation.configuration.len(),
        16384 - repair.body.len()
    );
    json!({"name":name,"pack":pack,"configuration":std::str::from_utf8(&generation.configuration).unwrap(),
        "generation_request":std::str::from_utf8(&generation.body).unwrap(),"generation_binding":generation.digest,
        "repair_request":std::str::from_utf8(&repair.body).unwrap(),"repair_binding":repair.digest,
        "utf8_bytes":{"pack":pack.len(),"configuration":generation.configuration.len(),"generation":generation.body.len(),"repair":repair.body.len()}})
}
pub fn restricted_wire_checks(rec: &RunRecord, p: &Prepared) {
    use inference::Broker;
    let mut broker = broker(rec, p);
    let reservation = broker.reserve(&p.scope, p.pack.utf8().as_bytes()).unwrap();
    let bound = request::bind(p.pack.utf8().as_bytes(), None).unwrap();
    let scope = &p.scope;
    let mut client = native::client("DW_INFERENCE_BROKER_DATABASE_URL");
    let invoke = |tx: &mut postgres::Transaction<'_>, b: &request::BoundCall, call: Uuid| {
        tx.query_one(
            "SELECT execution.inference_may_send($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &scope.engagement,
                &scope.campaign,
                &scope.decision,
                &call,
                &std::str::from_utf8(&b.pack).unwrap(),
                &std::str::from_utf8(&b.configuration).unwrap(),
                &std::str::from_utf8(&b.body).unwrap(),
                &b.digest,
            ],
        )
    };
    for field in [
        "permission_id",
        "mission_revision",
        "goal_ref",
        "scope",
        "question",
    ] {
        let mut pack: Value = serde_json::from_slice(&bound.pack).unwrap();
        pack[field] = if field == "mission_revision" {
            json!(2)
        } else {
            Value::Null
        };
        let forged = request::bind(&serde_json::to_vec(&pack).unwrap(), None).unwrap();
        let mut tx = client.transaction().unwrap();
        assert!(
            invoke(&mut tx, &forged, reservation.generation_call).is_err(),
            "SQL accepted forged pack {field}"
        );
        tx.rollback().unwrap();
    }
    for key in [
        "profile",
        "recipient",
        "protocol",
        "credential_reference",
        "requested_settings",
        "limits",
        "schema",
        "transport",
    ] {
        let mut config: Value = serde_json::from_slice(&bound.configuration).unwrap();
        config[key] = if config[key].is_null() {
            json!("substituted")
        } else {
            Value::Null
        };
        let cfg = serde_json::to_vec(&config).unwrap();
        let forged = request::BoundCall {
            pack: bound.pack.clone(),
            digest: request::compute_binding(&bound.pack, &cfg, &bound.body),
            configuration: cfg,
            body: bound.body.clone(),
        };
        let mut tx = client.transaction().unwrap();
        assert!(
            invoke(&mut tx, &forged, reservation.generation_call).is_err(),
            "SQL accepted forged {key}"
        );
        tx.rollback().unwrap();
    }
    for (key, value) in [
        ("reasoning", json!({"effort":"synthetic-forged-effort"})),
        ("max_output_tokens", json!(8192)),
        ("model", json!("synthetic-forged-model")),
    ] {
        let mut config: Value = serde_json::from_slice(&bound.configuration).unwrap();
        config["requested_settings"][key] = value;
        let cfg = serde_json::to_vec(&config).unwrap();
        let forged = request::BoundCall {
            pack: bound.pack.clone(),
            digest: request::compute_binding(&bound.pack, &cfg, &bound.body),
            configuration: cfg,
            body: bound.body.clone(),
        };
        let mut tx = client.transaction().unwrap();
        assert!(
            invoke(&mut tx, &forged, reservation.generation_call).is_err(),
            "SQL accepted forged requested setting {key}"
        );
        tx.rollback().unwrap();
    }
    // A configuration rebound to another admitted profile grants no send here.
    let mut config: Value = serde_json::from_slice(&bound.configuration).unwrap();
    config["profile"] = json!(
        if config["profile"] == "deepseek-responses-low-v1" {
            "deepseek-responses-reasoning-v1"
        } else {
            "deepseek-responses-low-v1"
        }
    );
    let cfg = serde_json::to_vec(&config).unwrap();
    let forged = request::BoundCall {
        pack: bound.pack.clone(),
        digest: request::compute_binding(&bound.pack, &cfg, &bound.body),
        configuration: cfg,
        body: bound.body.clone(),
    };
    let mut tx = client.transaction().unwrap();
    assert!(
        invoke(&mut tx, &forged, reservation.generation_call).is_err(),
        "SQL accepted a cross-profile rebound configuration"
    );
    tx.rollback().unwrap();
    let repair = request::bind(&bound.pack, Some(&["unexpected_view_request"; 8])).unwrap();
    let mut tx = client.transaction().unwrap();
    assert!(
        invoke(&mut tx, &repair, reservation.repair_call).is_err(),
        "repair requires completed original generation"
    );
    tx.rollback().unwrap();
    let mut tx = client.transaction().unwrap();
    invoke(&mut tx, &bound, reservation.generation_call).unwrap();
    tx.query_one(
        "SELECT execution.inference_call_state($1,'completed',$2)",
        &[
            &reservation.generation_call,
            &json!({"input_tokens":null,"output_tokens":null}),
        ],
    )
    .unwrap();
    invoke(&mut tx, &repair, reservation.repair_call).unwrap();
    tx.rollback().unwrap();
    // Wire admission is rolled back; this binding-only case publishes no decision.
}
