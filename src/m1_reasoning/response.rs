//! Transient provider admission; only bounded safe fields escape.
use super::bounded_json::JsonFailureKind;
use super::inference::{
    IncompleteReason, ProviderReport, Received, ResponseMetadata, ResponseStatus, Usage,
    UsageAvailability,
};
use super::proposal::{self, ProposalBasis};
use super::provider_diagnostic::{
    Classification, ErrorParam, ErrorType, Phase, ProviderDiagnostic,
};
use std::cell::Cell;
use std::time::Duration;
type Res<T> = Result<T, &'static str>;
// libcurl owns transfer framing; this admits only the bounded header/body callback bytes.
fn status_line(line: &str) -> Res<u16> {
    let status = line
        .get(9..12)
        .ok_or("provider_protocol")?
        .parse::<u16>()
        .map_err(|_| "provider_protocol")?;
    let valid = [
        line.starts_with("HTTP/1.1 "),
        line.as_bytes().get(12).is_none_or(|b| *b == b' '),
        !invalid_controls(line),
        (100..600).contains(&status),
    ];
    if valid.into_iter().all(|ok| ok) {
        Ok(status)
    } else {
        Err("provider_protocol")
    }
}
fn invalid_controls(text: &str) -> bool {
    text.bytes().any(|b| b.is_ascii_control() && b != b'\t')
}
fn header_line(line: &str, d: &mut ProviderDiagnostic) -> Res<()> {
    let (name, value) = line.split_once(':').ok_or("provider_protocol")?;
    let token = !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b));
    if !token || invalid_controls(value) {
        return Err("provider_protocol");
    }
    if name.eq_ignore_ascii_case("content-encoding")
        && !value.trim().eq_ignore_ascii_case("identity")
    {
        d.classification = Classification::UnsupportedEncoding;
        return Err("provider_protocol");
    }
    Ok(())
}
fn header_block(raw: &[u8], d: &mut ProviderDiagnostic) -> Res<(u16, usize)> {
    let end = raw
        .windows(4)
        .position(|v| v == b"\r\n\r\n")
        .ok_or("provider_protocol")?;
    let headers = std::str::from_utf8(&raw[..end]).map_err(|_| "provider_protocol")?;
    let mut lines = headers.split("\r\n");
    let status = status_line(lines.next().ok_or("provider_protocol")?)?;
    if status >= 200 {
        d.final_http_status = Some(status);
    }
    lines.try_for_each(|line| header_line(line, d))?;
    Ok((status, end + 4))
}
fn error_metadata(raw: &[u8], d: &mut ProviderDiagnostic) {
    if let Ok(v) = super::bounded_json::parse(raw, 65536, 65536) {
        d.error_type =
            Some(serde_json::from_value(v["error"]["type"].clone()).unwrap_or(ErrorType::Unknown));
        d.error_param = Some(
            serde_json::from_value(v["error"]["param"].clone()).unwrap_or(ErrorParam::Unknown),
        );
    }
}
fn http_body<'a>(raw: &'a [u8], d: &mut ProviderDiagnostic) -> Res<&'a [u8]> {
    d.phase = Phase::Http;
    d.classification = Classification::InvalidHeaders;
    let mut body = raw;
    loop {
        let (status, used) = header_block(body, d)?;
        body = &body[used..];
        match status {
            101 => {
                d.classification = Classification::UpgradeRefused;
                return Err("provider_protocol");
            }
            100..200 => {
                d.interim_responses += 1;
            }
            200 => return Ok(body),
            _ => {
                d.classification = Classification::HttpError;
                error_metadata(body, d);
                return Err("provider_protocol");
            }
        }
    }
}

pub fn admit_http(
    raw: &[u8],
    elapsed: Duration,
    basis: &mut dyn ProposalBasis,
    d: &mut ProviderDiagnostic,
) -> Res<Received> {
    d.report = None;
    d.response_bytes = raw.len();
    if raw.len() > 65536 {
        d.classification = Classification::ResponseLimit;
        return Err("provider_response_limit");
    }
    let body = http_body(raw, d)?;
    admit_json(body, raw.len(), elapsed, basis, d)
}
pub fn admit_envelope(
    raw: &[u8],
    response_bytes: usize,
    elapsed: Duration,
    basis: &mut dyn ProposalBasis,
) -> Res<Received> {
    let mut d = ProviderDiagnostic::default();
    admit_json(raw, response_bytes, elapsed, basis, &mut d)
}
fn admit_json(
    raw: &[u8],
    response_bytes: usize,
    elapsed: Duration,
    basis: &mut dyn ProposalBasis,
    d: &mut ProviderDiagnostic,
) -> Res<Received> {
    let v = parse_envelope(raw, d)?;
    let report = report(&v, response_bytes, elapsed);
    let usage = report.usage.clone();
    d.report = Some(report);
    let text = output_text(&v, d)?;
    let admission = proposal::admit(text.as_bytes(), basis);
    d.phase = Phase::Completion;
    d.classification = Classification::Completed;
    Ok(Received {
        admission,
        accounting_confirmed: false,
        usage,
    })
}
fn counter(v: &serde_json::Value) -> Option<u64> {
    v.as_u64().filter(|n| *n <= i64::MAX as u64)
}
fn response_status(v: &serde_json::Value) -> ResponseStatus {
    if v["status"].is_null() {
        return ResponseStatus::Missing;
    }
    match v["status"].as_str() {
        Some("completed") => ResponseStatus::Completed,
        Some("incomplete") => ResponseStatus::Incomplete,
        Some("failed") => ResponseStatus::Failed,
        Some("in_progress") => ResponseStatus::InProgress,
        _ => ResponseStatus::Unrecognized,
    }
}
fn report(v: &serde_json::Value, response_bytes: usize, elapsed: Duration) -> ProviderReport {
    let input_tokens = counter(&v["usage"]["input_tokens"]);
    let output_tokens = counter(&v["usage"]["output_tokens"]);
    let reasoning_tokens = counter(&v["usage"]["output_tokens_details"]["reasoning_tokens"]);
    let available = [input_tokens, output_tokens, reasoning_tokens]
        .iter()
        .filter(|n| n.is_some())
        .count();
    let usage_availability = match available {
        3 => UsageAvailability::Reported,
        0 => UsageAvailability::Unavailable,
        _ => UsageAvailability::Partial,
    };
    let incomplete_reason =
        (v["status"] == "incomplete").then(|| match v["incomplete_details"]["reason"].as_str() {
            Some("max_output_tokens") => IncompleteReason::MaxOutputTokens,
            Some("content_filter") => IncompleteReason::ContentFilter,
            _ => IncompleteReason::Unknown,
        });
    ProviderReport {
        usage: Usage {
            input_tokens,
            output_tokens,
            reasoning_tokens,
            response_bytes,
            wall_time_ms: elapsed.as_millis().min(i64::MAX as u128) as u64,
            requested_model: "deepseek-flash",
            reported_model: v["model"]
                .as_str()
                .filter(|m| {
                    matches!(
                        *m,
                        "deepseek-flash" | "probe-fixture" | "protocol-fixture-v1"
                    )
                })
                .map(str::to_owned),
        },
        provider_response: ResponseMetadata {
            status: response_status(v),
            incomplete_reason,
            usage_availability,
        },
    }
}
fn parse_envelope(raw: &[u8], d: &mut ProviderDiagnostic) -> Res<serde_json::Value> {
    d.phase = Phase::Json;
    d.classification = Classification::InvalidJson;
    super::bounded_json::parse_envelope_detailed(raw).map_err(|failure| {
        d.classification = match failure.kind {
            JsonFailureKind::Size
            | JsonFailureKind::Depth
            | JsonFailureKind::Collection
            | JsonFailureKind::KeyBytes
            | JsonFailureKind::TextBytes
            | JsonFailureKind::NumericRange => Classification::JsonBoundExceeded,
            JsonFailureKind::DuplicateKey | JsonFailureKind::Data => {
                Classification::JsonStructureRefused
            }
            JsonFailureKind::Syntax | JsonFailureKind::Eof | JsonFailureKind::Trailing => {
                Classification::InvalidJson
            }
        };
        d.json_error = Some(failure);
        "provider_protocol"
    })
}
fn message_text(item: &serde_json::Value) -> Res<&str> {
    if item["role"] != "assistant" || item["status"] != "completed" {
        return Err("provider_protocol");
    }
    let content = item["content"].as_array().ok_or("provider_protocol")?;
    if content.len() != 1 || content[0]["type"] != "output_text" {
        return Err("provider_protocol");
    }
    content[0]["text"].as_str().ok_or("provider_protocol")
}
fn output_text<'a>(v: &'a serde_json::Value, d: &mut ProviderDiagnostic) -> Res<&'a str> {
    d.phase = Phase::Envelope;
    d.classification = Classification::EnvelopeRejected;
    if v["status"] != "completed" {
        return Err("provider_incomplete");
    }
    let output = v["output"].as_array().ok_or("provider_protocol")?;
    assistant_text(output)
}
fn assistant_text(output: &[serde_json::Value]) -> Res<&str> {
    let mut text = None;
    for item in output {
        match item["type"].as_str() {
            Some("reasoning") => {} // private reasoning never escapes this boundary
            Some("message") => {
                if text.is_some() {
                    return Err("provider_protocol");
                }
                text = Some(message_text(item)?);
            }
            _ => return Err("provider_protocol"),
        }
    }
    text.ok_or("provider_protocol")
}

struct KeyExclusion<'a> {
    owner: &'a mut dyn ProposalBasis,
    key: &'a str,
    reflected: Cell<bool>,
}
impl KeyExclusion<'_> {
    fn clear(&self, text: &str) -> bool {
        if text.contains(self.key) {
            self.reflected.set(true);
            return false;
        }
        true
    }
}
impl ProposalBasis for KeyExclusion<'_> {
    fn reference_eligible(&self, r: &str) -> bool {
        self.clear(r) && self.owner.reference_eligible(r)
    }
    fn manifest_eligible(&self, r: &str) -> bool {
        self.clear(r) && self.owner.manifest_eligible(r)
    }
    fn safe_explanation(&self, text: &str) -> bool {
        self.clear(text) && self.owner.safe_explanation(text)
    }
    fn current_authority_and_premises(&mut self) -> Res<()> {
        self.owner.current_authority_and_premises()
    }
    fn current_authority_for_references(&mut self, refs: &[String]) -> Res<()> {
        if !refs.iter().all(|r| self.clear(r)) {
            return Err("provider_unsafe_response");
        }
        self.owner.current_authority_for_references(refs)
    }
}
/// One key-exclusion owner shared by the retained HTTP and SDK adapters.
pub(super) fn screened(
    key: &str,
    basis: &mut dyn ProposalBasis,
    diagnostic: &mut ProviderDiagnostic,
    admit: impl FnOnce(&mut dyn ProposalBasis, &mut ProviderDiagnostic) -> Res<Received>,
) -> Res<Received> {
    let mut screened = KeyExclusion {
        owner: basis,
        key,
        reflected: Cell::new(false),
    };
    let result = admit(&mut screened, diagnostic);
    let reflected_report = diagnostic
        .report
        .as_ref()
        .is_some_and(|r| serde_json::to_string(r).map_or(true, |s| s.contains(key)));
    if screened.reflected.get() || reflected_report {
        diagnostic.report = None;
        return Err("provider_unsafe_response");
    }
    result
}

/// SDK content remains the proposal source; the existing envelope gate preserves
/// terminal metadata and verifies that no unsupported output was silently lost.
pub(super) fn admit_sdk(
    raw: &[u8],
    content: Option<&str>,
    decoded: bool,
    elapsed: Duration,
    basis: &mut dyn ProposalBasis,
    d: &mut ProviderDiagnostic,
) -> Res<Received> {
    let v = parse_envelope(raw, d)?;
    let report = report(&v, raw.len(), elapsed);
    let usage = report.usage.clone();
    d.report = Some(report);
    if !decoded {
        return Err("provider_protocol");
    }
    let original = output_text(&v, d)?;
    if content != Some(original) {
        return Err("provider_protocol");
    }
    let admission = proposal::admit(content.ok_or("provider_protocol")?.as_bytes(), basis);
    d.phase = Phase::Completion;
    d.classification = Classification::Completed;
    Ok(Received {
        admission,
        usage,
        accounting_confirmed: false,
    })
}
