//! Successor HTTPS-loop ContextPack composition and opaque `endpoint_N` projection.
mod packing;
use super::terrain_projection::{
    append_https_dns_references, attach_candidate_refs, endpoint_ref_json, https_ref_json,
};
use super::*;
use crate::m1_autonomous_context::AutonomousContext;
use crate::m1_https::{
    DecisionEndpointMap, EndpointBinding, HTTPS_METHOD_HEAD, HTTPS_PORT, INTENT_DNS_UDP,
    INTENT_HTTPS_HEAD_V1, IssuedAssetBinding, MAX_ENDPOINTS_PER_HOST, VERSION_ENDPOINT_MAP,
    VERSION_HTTPS_LOOP, sha256_hex,
};
use crate::m1_https_terrain::TerrainHttpsClaimView;
use crate::m1_terrain_view::TerrainClaimSummary;
use serde_json::{Value, json};
use uuid::Uuid;

pub const HTTPS_LOOP_QUESTION: &str = "Select next authorized candidate for dns_udp or single-endpoint https-head-v1 (endpoint_N), or retain/defer/stop/capability_gap. Use issued IDs only; HEAD tests one endpoint and never proves whole-host reachability or objective completion.";

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnsEndpointMaterial {
    pub asset_ref: AssetRef,
    pub hostname: String,
    pub claim_id: Uuid,
    pub claim_revision: u32,
    pub vantage_ref: Uuid,
    pub resolver_ipv4: String,
    pub perceived_at: i64,
    pub lowest_ttl_seconds: Option<u32>,
    pub valid_until: i64,
    pub effective_status: String,
    pub conflicted: bool,
    pub integrity_hold: bool,
    pub answers: Vec<String>,
}

pub struct HttpsContextBasis {
    pub basis: ContextBasis,
    pub request_id: Uuid,
    pub dns_material: Vec<DnsEndpointMaterial>,
    pub https_claims: Vec<TerrainHttpsClaimView>,
}

pub trait HttpsContextReader {
    fn read_https_loop(
        &mut self,
        q: &ContextRequest,
        op: OperationId,
        run_id: Uuid,
    ) -> Res<HttpsContextBasis>;
}

pub struct PreparedHttpsLoop {
    pub prepared: PreparedContext,
    pub endpoint_map: DecisionEndpointMap,
}

fn check_loop_request(
    q: &ContextRequest,
    op: OperationId,
    run_id: Uuid,
    a: &AutonomousContext,
) -> Res<()> {
    q.validate(op)?;
    let fixture = a.binding.run_profile == VERSION_HTTPS_LOOP
        && a.binding.execution_mode == "controlled_fixture";
    let lab = a.binding.run_profile == crate::m1_inference_profile::HTTPS_LAB_RUN_PROFILE
        && a.binding.execution_mode == "live";
    (!run_id.is_nil() && (fixture || lab))
        .then_some(())
        .ok_or(Fail::State("context_basis_mismatch"))
}

pub fn prepare(
    reader: &mut impl HttpsContextReader,
    q: &ContextRequest,
    op: OperationId,
    run_id: Uuid,
    a: &AutonomousContext,
) -> Res<PreparedHttpsLoop> {
    check_loop_request(q, op, run_id, a)?;
    let ctx = reader.read_https_loop(q, op, run_id)?;
    check_autonomous_basis(&ctx.basis, a)?;
    let disclosure = check_basis(&ctx.basis, q)?;
    let terrain = resolve_terrain(&ctx.basis, &disclosure, q)?;
    packing::fit_https_pack(&ctx, &disclosure, q, op, &terrain, a)
}

pub(super) fn build_endpoint_map(
    ctx: &HttpsContextBasis,
    op: OperationId,
    assets: Vec<IssuedAssetBinding>,
    bindings: Vec<EndpointBinding>,
    utf8: &[u8],
) -> Res<DecisionEndpointMap> {
    let map = DecisionEndpointMap {
        version: VERSION_ENDPOINT_MAP.into(),
        engagement_id: ctx.basis.mission.engagement_id,
        campaign_id: ctx.basis.mission.campaign_id,
        decision_id: op,
        request_id: ctx.request_id,
        pack_digest: sha256_hex(utf8),
        pack_revision: 1,
        assets,
        bindings,
    };
    map.validate()?;
    Ok(map)
}

pub(super) fn endpoint_parent_dependencies(
    terrain: &TerrainContextState,
    endpoints: &[EndpointBinding],
) -> Res<Vec<TerrainDependency>> {
    if endpoints.is_empty() {
        return Ok(Vec::new());
    }
    let TerrainContextState::Available(view) = terrain else {
        return Err(Fail::State("context_basis_mismatch"));
    };
    let mut deps = Vec::with_capacity(endpoints.len());
    for ep in endpoints {
        let claim = view
            .claims
            .iter()
            .find(|c| c.claim_id == ep.parent_claim_id)
            .ok_or(Fail::State("context_basis_mismatch"))?;
        deps.push(TerrainDependency::from_endpoint_parent(
            ep.local_id.clone(),
            claim,
        )?);
    }
    Ok(deps)
}

pub(super) fn loop_dns_claims(
    terrain: &TerrainContextState,
    cands: &[Candidate],
) -> Vec<TerrainClaimSummary> {
    let TerrainContextState::Available(view) = terrain else {
        return Vec::new();
    };
    view.claims
        .iter()
        .filter(|c| {
            c.effective_status == EffectiveClaimStatus::Observed
                && !c.conflicted
                && !c.integrity_hold
                && !c.corroborated
                && cands.iter().any(|cand| cand.asset_ref == c.asset_ref)
        })
        .cloned()
        .collect()
}

pub(super) fn select_endpoints(
    ctx: &HttpsContextBasis,
    cands: &[Candidate],
) -> Res<(Vec<EndpointBinding>, usize, usize)> {
    let Some(perm) = ctx.basis.mission.fields.m1_permission() else {
        return Ok((Vec::new(), 0, 0));
    };
    let mut bindings = Vec::new();
    let mut total_answers = 0usize;
    let mut chosen_host: Option<String> = None;
    for mat in ctx
        .dns_material
        .iter()
        .filter(|m| material_eligible(m, cands, ctx.basis.evaluated_at))
    {
        let Some(path) = matching_contact_path(perm, mat, &mut chosen_host) else {
            continue;
        };
        total_answers = total_answers.saturating_add(mat.answers.len());
        append_material_bindings(&mut bindings, mat, path);
    }
    let omitted = total_answers.saturating_sub(bindings.len());
    Ok((bindings, omitted, total_answers))
}

fn matching_contact_path<'a>(
    perm: &'a crate::m1_permission::M1Permission,
    mat: &DnsEndpointMaterial,
    chosen_host: &mut Option<String>,
) -> Option<&'a str> {
    let (path, vantage, resolver) =
        perm.https_contact_tuple(mat.asset_ref, &mat.hostname, mat.perceived_at)?;
    if vantage != mat.vantage_ref || resolver.to_string() != mat.resolver_ipv4 {
        return None;
    }
    if chosen_host.as_deref().is_some_and(|h| h != mat.hostname) {
        return None;
    }
    *chosen_host = Some(mat.hostname.clone());
    Some(path)
}

pub(super) fn append_material_bindings(
    bindings: &mut Vec<EndpointBinding>,
    mat: &DnsEndpointMaterial,
    path: &str,
) {
    for ip in &mat.answers {
        if bindings.len() >= MAX_ENDPOINTS_PER_HOST {
            break;
        }
        if bindings.iter().any(|b| b.ipv4 == *ip) {
            continue;
        }
        bindings.push(EndpointBinding {
            local_id: format!("endpoint_{}", bindings.len()),
            asset_ref: mat.asset_ref,
            hostname: mat.hostname.clone(),
            ipv4: ip.clone(),
            port: HTTPS_PORT,
            approved_path: path.to_owned(),
            method: HTTPS_METHOD_HEAD.into(),
            parent_claim_id: mat.claim_id,
            parent_claim_revision: mat.claim_revision,
            resolver_ipv4: mat.resolver_ipv4.clone(),
            vantage_ref: mat.vantage_ref,
            perceived_at: mat.perceived_at,
            lowest_ttl_seconds: mat.lowest_ttl_seconds,
            valid_until: mat.valid_until,
        });
    }
}

fn material_eligible(mat: &DnsEndpointMaterial, cands: &[Candidate], evaluated_at: i64) -> bool {
    cands.iter().any(|c| c.asset_ref == mat.asset_ref)
        && mat.effective_status == "observed"
        && !mat.conflicted
        && !mat.integrity_hold
        && dns_ttl_valid(mat, evaluated_at)
}

fn dns_ttl_valid(mat: &DnsEndpointMaterial, evaluated_at: i64) -> bool {
    mat.lowest_ttl_seconds.is_some_and(|t| t > 0)
        && mat.claim_revision >= 1
        && evaluated_at >= mat.perceived_at
        && mat.valid_until >= mat.perceived_at
}

pub(super) fn filter_https_claims<'a>(
    claims: &'a [TerrainHttpsClaimView],
    cands: &[Candidate],
) -> Vec<&'a TerrainHttpsClaimView> {
    claims
        .iter()
        .filter(|h| cands.iter().any(|c| c.asset_ref == h.asset_ref))
        .collect()
}

pub(super) fn issued_assets(cands: &[Candidate]) -> Vec<IssuedAssetBinding> {
    cands
        .iter()
        .enumerate()
        .map(|(i, c)| IssuedAssetBinding {
            local_id: format!("asset_{i}"),
            asset_ref: c.asset_ref,
        })
        .collect()
}

#[expect(
    clippy::too_many_arguments,
    reason = "local projection writer keeps extension assembly shallow"
)]
pub(super) fn apply_https_extensions(
    raw: &mut Value,
    ctx: &HttpsContextBasis,
    a: &AutonomousContext,
    endpoints: &[EndpointBinding],
    deps: &[TerrainDependency],
    https_claims: &[&TerrainHttpsClaimView],
    omitted_eps: usize,
    total_eps: usize,
    omitted_cands: usize,
) -> Res<()> {
    raw["version"] = json!(VERSION_HTTPS_LOOP);
    raw["question"] = json!(HTTPS_LOOP_QUESTION);
    raw["manifests"] = manifests_for_binding(a, endpoints);
    let refs = raw["references"]
        .as_array_mut()
        .ok_or(Fail::Store("context_encode"))?;
    refs.extend(
        endpoints
            .iter()
            .map(|ep| endpoint_ref_json(ep, ctx.basis.evaluated_at)),
    );
    refs.extend(
        https_claims
            .iter()
            .enumerate()
            .map(|(i, h)| https_ref_json(i, h, endpoints, ctx.basis.evaluated_at)),
    );
    if omitted_eps > 0 || total_eps >= MAX_ENDPOINTS_PER_HOST {
        raw["omissions"]["endpoints"] = json!({
            "dns_answers_total": total_eps, "shown_endpoints": endpoints.len(),
            "omitted_endpoints": omitted_eps, "partial_coverage": true
        });
    }
    if omitted_cands > 0 {
        raw["omissions"]["candidates_omitted_by_byte_bound"] = json!(omitted_cands);
    }
    attach_candidate_refs(raw, endpoints, deps, https_claims)
}

fn manifests_for_binding(a: &AutonomousContext, endpoints: &[EndpointBinding]) -> Value {
    let head_avail = if endpoints.is_empty() {
        "requires_dns_premise"
    } else {
        "qualified"
    };
    if a.binding.execution_mode == "live" {
        return json!([
            {
                "availability": "qualified",
                "execution_mode": "live",
                "profile": crate::m1_dns::PROFILE,
                "qualification_ref": a.binding.qualification_ref,
                "reference": INTENT_DNS_UDP,
                "release_sha256": a.binding.release_sha256,
                "schema": crate::m1_dns::SCHEMA,
                "transport": "udp"
            },
            {
                "availability": head_avail,
                "execution_mode": "live",
                "profile": crate::m1_inference_profile::HTTPS_HEAD_PROFILE,
                "qualification_ref": a.binding.qualification_ref,
                "reference": INTENT_HTTPS_HEAD_V1,
                "release_sha256": a.binding.release_sha256,
                "schema": crate::m1_https::VERSION_HTTPS_IPC,
                "transport": "tcp_tls_http11_head"
            }
        ]);
    }
    json!([
        {"availability":"qualified","execution_mode":"controlled_fixture","profile":"dns-routing-fixture-v1",
         "reference":INTENT_DNS_UDP,"schema":"m1_dns_ipc_v1","transport":"udp"},
        {"availability":head_avail,
         "execution_mode":"controlled_fixture","profile":"https-head-fixture-v1",
         "reference":INTENT_HTTPS_HEAD_V1,"schema":"m1_https_head_request_v1","transport":"tcp_tls_http11_head"}
    ])
}
