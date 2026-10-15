use serde::Serialize;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::{cell::Cell, fmt};

// Owner-approved experiment: proposal/context ingress retains the existing bound.
pub const DEFAULT_JSON_DEPTH: usize = 8;
pub const RESPONSE_ENVELOPE_JSON_DEPTH: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonFailureKind {
    Size,
    Syntax,
    Eof,
    Depth,
    Collection,
    DuplicateKey,
    KeyBytes,
    TextBytes,
    NumericRange,
    Trailing,
    Data,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct JsonFailure {
    pub kind: JsonFailureKind,
    pub line: Option<usize>,
    pub column: Option<usize>,
}
impl JsonFailure {
    pub fn from_serde(
        error: &serde_json::Error,
        local: Option<JsonFailureKind>,
        input_bytes: usize,
    ) -> Self {
        let kind = local.unwrap_or_else(|| match error.classify() {
            // serde exposes no numeric-range code; match only its fixed internal label.
            // The transient message is discarded, never copied into this error value.
            serde_json::error::Category::Syntax
                if error
                    .to_string()
                    .starts_with("number out of range at line ") =>
            {
                JsonFailureKind::NumericRange
            }
            serde_json::error::Category::Syntax => JsonFailureKind::Syntax,
            serde_json::error::Category::Eof => JsonFailureKind::Eof,
            serde_json::error::Category::Data | serde_json::error::Category::Io => {
                JsonFailureKind::Data
            }
        });
        let position = |n| (n > 0 && n <= input_bytes.saturating_add(1)).then_some(n);
        Self {
            kind,
            line: position(error.line()),
            column: position(error.column()),
        }
    }
}
fn refusal<E: de::Error>(failure: &Cell<Option<JsonFailureKind>>, kind: JsonFailureKind) -> E {
    failure.set(Some(kind));
    E::custom("bounded_json_refused")
}
#[derive(Clone, Copy)]
struct Seed<'a> {
    depth: usize,
    depth_limit: usize,
    text_bytes: usize,
    full: bool,
    failure: &'a Cell<Option<JsonFailureKind>>,
}

struct KeySeed<'a> {
    full: bool,
    failure: &'a Cell<Option<JsonFailureKind>>,
}

pub fn parse(raw: &[u8], bytes: usize, text_bytes: usize) -> Result<Value, &'static str> {
    parse_detailed(raw, bytes, text_bytes).map_err(|failure| match failure.kind {
        JsonFailureKind::Size => "json_size",
        JsonFailureKind::Trailing => "json_trailing",
        _ => "json_structure",
    })
}
pub fn parse_detailed(raw: &[u8], bytes: usize, text_bytes: usize) -> Result<Value, JsonFailure> {
    parse_with_depth(raw, bytes, text_bytes, DEFAULT_JSON_DEPTH)
}
/// Experimental provider-envelope profile; all other ingress keeps the original bound.
pub fn parse_envelope_detailed(raw: &[u8]) -> Result<Value, JsonFailure> {
    parse_with_depth(raw, 65536, 65536, RESPONSE_ENVELOPE_JSON_DEPTH)
}
fn parse_with_depth(
    raw: &[u8],
    bytes: usize,
    text_bytes: usize,
    depth_limit: usize,
) -> Result<Value, JsonFailure> {
    if raw.is_empty() || raw.len() > bytes {
        return Err(JsonFailure {
            kind: JsonFailureKind::Size,
            line: None,
            column: None,
        });
    }
    let failure = Cell::new(None);
    let mut parser = serde_json::Deserializer::from_slice(raw);
    let value = Seed {
        depth: 0,
        depth_limit,
        text_bytes,
        full: false,
        failure: &failure,
    }
    .deserialize(&mut parser)
    .map_err(|error| JsonFailure::from_serde(&error, failure.get(), raw.len()))?;
    parser.end().map_err(|error| {
        JsonFailure::from_serde(&error, Some(JsonFailureKind::Trailing), raw.len())
    })?;
    Ok(value)
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        if self.full {
            return Err(refusal(self.failure, JsonFailureKind::Collection));
        }
        if self.depth >= self.depth_limit {
            return Err(refusal(self.failure, JsonFailureKind::Depth));
        }
        d.deserialize_any(self)
    }
}

impl Seed<'_> {
    fn nested(self, full: bool) -> Self {
        Self {
            depth: self.depth + 1,
            depth_limit: self.depth_limit,
            text_bytes: self.text_bytes,
            full,
            failure: self.failure,
        }
    }
}

impl<'de> DeserializeSeed<'de> for KeySeed<'_> {
    type Value = String;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<String, D::Error> {
        if self.full {
            return Err(refusal(self.failure, JsonFailureKind::Collection));
        }
        d.deserialize_str(self)
    }
}

impl<'de> Visitor<'de> for KeySeed<'_> {
    type Value = String;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("one bounded JSON object key")
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<String, E> {
        if v.len() > 64 {
            return Err(refusal(self.failure, JsonFailureKind::KeyBytes));
        }
        Ok(v.into())
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<String, E> {
        if v.len() > 64 {
            return Err(refusal(self.failure, JsonFailureKind::KeyBytes));
        }
        Ok(v)
    }
}

impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("one bounded JSON value")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| refusal(self.failure, JsonFailureKind::NumericRange))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        if v.len() > self.text_bytes {
            return Err(refusal(self.failure, JsonFailureKind::TextBytes));
        }
        Ok(Value::String(v.into()))
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        if v.len() > self.text_bytes {
            return Err(refusal(self.failure, JsonFailureKind::TextBytes));
        }
        Ok(Value::String(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        if self.depth + 1 >= self.depth_limit {
            return Err(refusal(self.failure, JsonFailureKind::Depth));
        }
        let mut values = Vec::new();
        while let Some(v) = a.next_element_seed(self.nested(values.len() == 64))? {
            values.push(v);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        if self.depth + 1 >= self.depth_limit {
            return Err(refusal(self.failure, JsonFailureKind::Depth));
        }
        let mut values = Map::new();
        while let Some(k) = a.next_key_seed(KeySeed {
            full: values.len() == 64,
            failure: self.failure,
        })? {
            if values.contains_key(&k) {
                return Err(refusal(self.failure, JsonFailureKind::DuplicateKey));
            }
            let v = a.next_value_seed(self.nested(false))?;
            values.insert(k, v);
        }
        Ok(Value::Object(values))
    }
}
