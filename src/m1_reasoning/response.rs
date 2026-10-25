//! Transient provider admission; only bounded safe fields escape.
use super::bounded_json::JsonFailureKind;
use super::inference::{Received, Usage};
use super::proposal::{self, ProposalBasis};
use super::provider_diagnostic::{
    Classification, ErrorParam, ErrorType, Phase, ProviderDiagnostic,
};
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
    let text = output_text(&v, d)?;
    let admission = proposal::admit(text.as_bytes(), basis);
    let usage = &v["usage"];
    d.phase = Phase::Completion;
    d.classification = Classification::Completed;
    Ok(Received {
        admission,
        accounting_confirmed: false,
        usage: Usage {
            input_tokens: usage["input_tokens"].as_u64(),
            output_tokens: usage["output_tokens"].as_u64(),
            reasoning_tokens: usage["output_tokens_details"]["reasoning_tokens"].as_u64(),
            response_bytes,
            wall_time_ms: elapsed.as_millis() as u64,
            requested_model: "deepseek-flash",
            // Only known safe metadata leaves the adapter; arbitrary model IDs do not.
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
    })
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
