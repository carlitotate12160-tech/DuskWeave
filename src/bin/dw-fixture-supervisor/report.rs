//! Safe publication of redacted action output and closed result metadata.
//!
//! Capture streams are redacted only after full assembly, so a secret split
//! across read chunks is still removed. A stream that overflowed the bounded
//! capture limit, or whose reader could not finish, is suppressed entirely
//! and published with a closed omission reason instead of partial content.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const REDACTED: &[u8] = b"[REDACTED]";
pub const CAPTURE_COMPLETE: &str = "complete";
pub const CAPTURE_INCOMPLETE: &str = "incomplete";
pub const CLEANUP_COMPLETED: &str = "completed";
pub const CLEANUP_UNKNOWN: &str = "unknown";

/// One captured output stream with its bounded-collection disposition.
pub struct CaptureStream {
    pub bytes: Vec<u8>,
    pub finished: bool,
    pub overflowed: bool,
}

/// Facts published as the closed `<label>.result.json` metadata record.
pub struct MetadataFields<'a> {
    pub status: &'a str,
    pub exit: i32,
    pub child_exit: Option<i32>,
    pub elapsed_ms: u64,
    pub action_budget_ms: u64,
    pub capture: &'a str,
    pub cleanup: &'a str,
    pub omissions: &'a [&'a str],
}

pub fn log_path(report_root: &Path, label: &str) -> PathBuf {
    report_root.join(format!("{label}.log"))
}

pub fn result_path(report_root: &Path, label: &str) -> PathBuf {
    report_root.join(format!("{label}.result.json"))
}

/// A collision is refused before any action work is launched.
pub fn collision(report_root: &Path, label: &str) -> bool {
    log_path(report_root, label).exists() || result_path(report_root, label).exists()
}

/// Replace every occurrence of each redaction value in assembled content.
pub fn redact(content: &[u8], redactions: &[String]) -> Vec<u8> {
    let mut output = content.to_vec();
    for secret in redactions {
        replace_all(&mut output, secret.as_bytes());
    }
    output
}

fn replace_all(content: &mut Vec<u8>, secret: &[u8]) {
    if secret.is_empty() {
        return;
    }
    let mut searched = 0;
    while let Some(at) = find_from(content, secret, searched) {
        let end = at + secret.len();
        content.splice(at..end, REDACTED.iter().copied());
        searched = at + REDACTED.len();
    }
}

fn find_from(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    let mut at = from;
    while at + needle.len() <= haystack.len() {
        if &haystack[at..at + needle.len()] == needle {
            return Some(at);
        }
        at += 1;
    }
    None
}

fn omission_reason(stream: &CaptureStream) -> &'static str {
    if stream.overflowed {
        "capture_overflow"
    } else {
        "capture_incomplete"
    }
}

/// Publishable form of one stream: redacted content or a closed omission.
pub fn stream_publication(
    stream: Option<&CaptureStream>,
    redactions: &[String],
    name: &str,
) -> Vec<u8> {
    let reason = match stream {
        Some(stream) if stream.finished && !stream.overflowed => {
            return redact(&stream.bytes, redactions);
        }
        Some(stream) => omission_reason(stream),
        None => "capture_incomplete",
    };
    format!("[{name} omitted: {reason}]").into_bytes()
}

fn collect_omission(
    stream: Option<&CaptureStream>,
    name: &'static str,
    omissions: &mut Vec<&'static str>,
) {
    let incomplete = match stream {
        Some(stream) => !stream.finished || stream.overflowed,
        None => true,
    };
    if incomplete {
        omissions.push(name);
    }
}

/// Closed capture disposition plus the names of suppressed streams.
pub fn capture_state(
    stdout: Option<&CaptureStream>,
    stderr: Option<&CaptureStream>,
) -> (&'static str, Vec<&'static str>) {
    let mut omissions = Vec::new();
    collect_omission(stdout, "stdout", &mut omissions);
    collect_omission(stderr, "stderr", &mut omissions);
    let state = if omissions.is_empty() {
        CAPTURE_COMPLETE
    } else {
        CAPTURE_INCOMPLETE
    };
    (state, omissions)
}

pub fn metadata_json(fields: &MetadataFields<'_>) -> String {
    json!({
        "status": fields.status,
        "exit": fields.exit,
        "child_exit": fields.child_exit,
        "elapsed_seconds": fields.elapsed_ms as f64 / 1000.0,
        "action_budget_ms": fields.action_budget_ms,
        "timing_origin": "request_admission",
        "capture": fields.capture,
        "cleanup": fields.cleanup,
        "omissions": fields.omissions,
    })
    .to_string()
}

/// Publish the redacted log and result metadata exactly once; any failure is
/// reported to the caller and can never manufacture a PASS result.
pub fn publish(report_root: &Path, label: &str, log: &[u8], metadata: &str) -> Result<(), ()> {
    if collision(report_root, label) || std::fs::create_dir_all(report_root).is_err() {
        return Err(());
    }
    std::fs::write(log_path(report_root, label), log)
        .and_then(|()| std::fs::write(result_path(report_root, label), metadata))
        .map_err(|_| ())
}

/// The closed single-line result protocol emitted on stdout.
pub fn refusal_line(category: &str) -> Value {
    json!({ "ok": false, "category": category })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(bytes: &[u8], finished: bool, overflowed: bool) -> Option<CaptureStream> {
        Some(CaptureStream {
            bytes: bytes.to_vec(),
            finished,
            overflowed,
        })
    }

    #[test]
    fn redacts_secret_split_across_chunk_boundaries() {
        let secret = "synthetic-secret-value";
        let mut assembled = Vec::new();
        assembled.extend_from_slice(b"prefix ");
        assembled.extend_from_slice(&secret.as_bytes()[..10]);
        assembled.extend_from_slice(&secret.as_bytes()[10..]);
        assembled.extend_from_slice(b" suffix ");
        assembled.extend_from_slice(secret.as_bytes());
        let redactions = vec![secret.to_string()];
        let output = String::from_utf8(redact(&assembled, &redactions)).expect("utf-8");
        assert_eq!(output, "prefix [REDACTED] suffix [REDACTED]");
    }

    #[test]
    fn redacts_multiple_values_in_list_order() {
        let content = b"alpha beta alpha".to_vec();
        let redactions = vec!["alpha".to_string(), "beta".to_string()];
        let output = String::from_utf8(redact(&content, &redactions)).expect("utf-8");
        assert_eq!(output, "[REDACTED] [REDACTED] [REDACTED]");
    }

    #[test]
    fn empty_redaction_values_match_nothing() {
        let content = b"unchanged".to_vec();
        let output = redact(&content, &[String::new()]);
        assert_eq!(output, content);
    }

    #[test]
    fn complete_streams_publish_without_omissions() {
        let (state, omissions) = capture_state(stream(b"out", true, false).as_ref(), None);
        assert_eq!((state, omissions.len()), (CAPTURE_INCOMPLETE, 1));
        let (state, omissions) = capture_state(
            stream(b"out", true, false).as_ref(),
            stream(b"err", true, false).as_ref(),
        );
        assert_eq!((state, omissions.len()), (CAPTURE_COMPLETE, 0));
    }

    #[test]
    fn overflowed_or_unfinished_streams_are_suppressed() {
        let redactions = vec!["secret".to_string()];
        let overflowed = stream(b"partial-secret", true, true).expect("stream");
        let marker = stream_publication(Some(&overflowed), &redactions, "stdout");
        assert_eq!(marker, b"[stdout omitted: capture_overflow]");
        let unfinished = stream(b"partial-secret", false, false).expect("stream");
        let marker = stream_publication(Some(&unfinished), &redactions, "stderr");
        assert_eq!(marker, b"[stderr omitted: capture_incomplete]");
        let abandoned = stream_publication(None, &redactions, "stdout");
        assert_eq!(abandoned, b"[stdout omitted: capture_incomplete]");
    }

    #[test]
    fn metadata_carries_closed_disposition_fields() {
        let fields = MetadataFields {
            status: "harness_timeout",
            exit: 124,
            child_exit: Some(1),
            elapsed_ms: 512,
            action_budget_ms: 500,
            capture: CAPTURE_COMPLETE,
            cleanup: CLEANUP_UNKNOWN,
            omissions: &[],
        };
        let value: Value = serde_json::from_str(&metadata_json(&fields)).expect("json");
        assert_eq!(value["status"], "harness_timeout");
        assert_eq!(value["exit"], 124);
        assert_eq!(value["child_exit"], 1);
        assert_eq!(value["elapsed_seconds"], 0.512);
        assert_eq!(value["timing_origin"], "request_admission");
    }

    #[test]
    fn publish_refuses_collisions_and_writes_once() {
        let root = std::env::temp_dir().join(format!(
            "dw-fs-report-{}-{}",
            std::process::id(),
            "collision"
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("report root");
        assert!(publish(&root, "label", b"log", "{}").is_ok());
        assert!(collision(&root, "label"));
        assert!(publish(&root, "label", b"other", "{}").is_err());
        assert_eq!(
            std::fs::read(log_path(&root, "label")).expect("log"),
            b"log"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
