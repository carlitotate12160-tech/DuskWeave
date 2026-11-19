//! Immutable Rust-produced semantic template artifact for `m1_https_loop_v1`.
use crate::m1_https::{VERSION_HTTPS_LOOP, sha256_hex};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const TEMPLATE_VERSION_HTTPS_LOOP: &str = "m1-https-loop-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpsLoopSemanticTemplate {
    pub template_version: String,
    pub schema_version: String,
    pub instructions: String,
    pub schema: Value,
}

/// Produce the canonical `m1_https_loop_v1` semantic template and SHA-256 digest from an immutable ContextPack.
pub fn semantic_template(
    pack: &[u8],
) -> Result<(HttpsLoopSemanticTemplate, Value, String), &'static str> {
    let p = super::parsed_pack(pack)?;
    if p["version"] != VERSION_HTTPS_LOOP {
        return Err("request_encode");
    }
    let (instructions, schema) = super::super::request::semantic_material(pack)?;
    let template = HttpsLoopSemanticTemplate {
        template_version: TEMPLATE_VERSION_HTTPS_LOOP.to_owned(),
        schema_version: VERSION_HTTPS_LOOP.to_owned(),
        instructions,
        schema,
    };
    let mut val = serde_json::to_value(&template).map_err(|_| "request_encode")?;
    val.sort_all_objects();
    let raw = serde_json::to_vec(&val).map_err(|_| "request_encode")?;
    Ok((template, val, sha256_hex(&raw)))
}
