//! The legacy checker's bounded Markdown-link subset; no network or renderer.
use std::collections::{BTreeMap, BTreeSet};

fn erase_blocks(input: &str, open: &str, close: &str) -> String {
    let mut bytes = input.as_bytes().to_vec();
    let mut cursor = 0;
    while let Some(start) = input[cursor..].find(open).map(|i| cursor + i) {
        let tail = start + open.len();
        let Some(end) = input[tail..].find(close).map(|i| tail + i + close.len()) else {
            break;
        };
        for byte in &mut bytes[start..end] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
        cursor = end;
    }
    // Whole UTF-8 blocks become ASCII spaces; all remaining bytes are intact.
    String::from_utf8_lossy(&bytes).into_owned()
}

fn target(body: &str) -> Option<String> {
    let end = body.find(char::is_whitespace).unwrap_or(body.len());
    let suffix = body[end..].trim();
    if end == 0 || !(suffix.is_empty() || suffix.starts_with('"') && suffix.ends_with('"')) {
        return None;
    }
    Some(body[..end].trim_matches(['<', '>']).to_owned())
}

pub(super) fn links(input: &str) -> Vec<(usize, String)> {
    let text = erase_blocks(&erase_blocks(input, "```", "```"), "<!--", "-->");
    text.match_indices("](")
        .filter_map(|(start, _)| {
            let open = text[..start].rfind('[')?;
            if text[open..start].contains(']') {
                return None;
            }
            let body_start = start + 2;
            let end = text[body_start..].find(')')? + body_start;
            let value = target(&text[body_start..end])?;
            let line = input[..start].bytes().filter(|b| *b == b'\n').count() + 1;
            Some((line, value))
        })
        .collect()
}

fn heading_label(heading: &str) -> String {
    let mut out = String::new();
    let mut tail = heading;
    while let Some(open) = tail.find('[') {
        let Some(label_end) = tail[open..].find("](").map(|i| open + i) else {
            break;
        };
        let Some(end) = tail[label_end + 2..].find(')').map(|i| label_end + 2 + i) else {
            break;
        };
        out.push_str(&tail[..open]);
        out.push_str(&tail[open + 1..label_end]);
        tail = &tail[end + 1..];
    }
    out.push_str(tail);
    out
}

pub(super) fn slug(heading: &str) -> String {
    heading_label(heading.trim())
        .to_lowercase()
        .chars()
        .filter(|c| {
            c.is_alphanumeric()
                || c.is_whitespace()
                || *c == '_'
                || *c == '-'
                || ('\u{c0}'..='\u{ffff}').contains(c)
        })
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

pub(super) fn anchors(input: &str) -> BTreeSet<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    let mut result = BTreeSet::new();
    for line in erase_blocks(input, "```", "```").lines() {
        let hashes = line.bytes().take_while(|b| *b == b'#').count();
        if !(1..=6).contains(&hashes) || !line[hashes..].starts_with(char::is_whitespace) {
            continue;
        }
        let base = slug(&line[hashes..]);
        let count = counts.entry(base.clone()).or_default();
        result.insert(if *count == 0 {
            base
        } else {
            format!("{base}-{count}")
        });
        *count += 1;
    }
    result
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn escaped(bytes: &[u8]) -> Option<u8> {
    if bytes.first() != Some(&b'%') {
        return None;
    }
    Some(hex(*bytes.get(1)?)? * 16 + hex(*bytes.get(2)?)?)
}

pub(super) fn unquote(input: &str) -> String {
    let mut out = Vec::new();
    let mut rest = input.as_bytes();
    while !rest.is_empty() {
        if let Some(byte) = escaped(rest) {
            out.push(byte);
            rest = &rest[3..];
        } else {
            out.push(rest[0]);
            rest = &rest[1..];
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
