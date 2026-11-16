//! The legacy checker's bounded Markdown-link subset; no network or renderer.
use std::collections::{BTreeMap, BTreeSet};

fn strip_fence_indent(line: &str) -> Option<&str> {
    let trimmed = line.strip_suffix('\r').unwrap_or(line);
    let lead = trimmed.bytes().take_while(|b| *b == b' ').count();
    (lead <= 3).then_some(&trimmed[lead..])
}

fn fence_marker_len(rest: &str) -> Option<(u8, usize)> {
    let ch = *rest.as_bytes().first()?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let len = rest.bytes().take_while(|b| *b == ch).count();
    (len >= 3).then_some((ch, len))
}

fn parse_fence_open(line: &str) -> Option<(u8, usize)> {
    let rest = strip_fence_indent(line)?;
    let (ch, len) = fence_marker_len(rest)?;
    if ch == b'`' && rest[len..].contains('`') {
        return None;
    }
    Some((ch, len))
}

fn is_fence_close(line: &str, open_ch: u8, open_len: usize) -> bool {
    let Some(rest) = strip_fence_indent(line) else {
        return false;
    };
    let Some((ch, len)) = fence_marker_len(rest) else {
        return false;
    };
    ch == open_ch && len >= open_len && rest[len..].trim().is_empty()
}

fn push_spaces(out: &mut String, text: &str) {
    for ch in text.chars() {
        out.push(if ch == '\n' { '\n' } else { ' ' });
    }
}

fn mask_prose_line(mut rest: &str, in_comment: &mut bool, out: &mut String) {
    while !rest.is_empty() {
        if *in_comment {
            match rest.find("-->") {
                Some(idx) => {
                    push_spaces(out, &rest[..idx + 3]);
                    *in_comment = false;
                    rest = &rest[idx + 3..];
                }
                None => {
                    push_spaces(out, rest);
                    return;
                }
            }
        } else {
            match rest.find("<!--") {
                Some(idx) => {
                    out.push_str(&rest[..idx]);
                    push_spaces(out, &rest[idx..idx + 4]);
                    *in_comment = true;
                    rest = &rest[idx + 4..];
                }
                None => {
                    out.push_str(rest);
                    return;
                }
            }
        }
    }
}

fn mask_non_prose(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut fence: Option<(u8, usize)> = None;
    let mut in_comment = false;
    for raw in input.split_inclusive('\n') {
        let (line, nl) = raw.strip_suffix('\n').map_or((raw, ""), |s| (s, "\n"));
        if let Some((ch, len)) = fence {
            push_spaces(&mut out, line);
            out.push_str(nl);
            if is_fence_close(line, ch, len) {
                fence = None;
            }
            continue;
        }
        if let Some(open) = (!in_comment).then(|| parse_fence_open(line)).flatten() {
            push_spaces(&mut out, line);
            out.push_str(nl);
            fence = Some(open);
            continue;
        }
        mask_prose_line(line, &mut in_comment, &mut out);
        out.push_str(nl);
    }
    out
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
    let text = mask_non_prose(input);
    text.match_indices("](")
        .filter_map(|(start, _)| {
            let open = text[..start].rfind('[')?;
            if text[open..start].contains(']') {
                return None;
            }
            let body_start = start + 2;
            let end = text[body_start..].find(')')? + body_start;
            let value = target(&text[body_start..end])?;
            let line = text[..start].bytes().filter(|b| *b == b'\n').count() + 1;
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
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '_' || *c == '-')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

pub(super) fn anchors(input: &str) -> BTreeSet<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    let mut result = BTreeSet::new();
    for line in mask_non_prose(input).lines() {
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
