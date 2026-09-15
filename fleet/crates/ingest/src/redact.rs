use crate::types::{
    IngestError, RedactedCategory, RedactionReceipt, MAX_JSON_DEPTH, MAX_JSON_LEAVES,
};
use serde_json::{Map, Value};

#[path = "redact_patterns.rs"]
mod redact_patterns;
use redact_patterns::{typed_category, SECRET_FIELDS};

fn walk(
    v: Value,
    depth: usize,
    leaves: &mut usize,
    r: &mut RedactionReceipt,
) -> Result<Value, IngestError> {
    if depth > MAX_JSON_DEPTH {
        return Err(IngestError::JsonTooDeep);
    }
    match v {
        Value::String(s) => {
            *leaves += 1;
            if *leaves > MAX_JSON_LEAVES {
                return Err(IngestError::JsonTooComplex);
            }
            if let Some(cat) = typed_category(&s) {
                r.categories.push(cat);
                r.field_count += 1;
                return Ok(Value::String(format!("[REDACTED:{cat:?}]")));
            }
            Ok(Value::String(s))
        }
        Value::Object(map) => {
            let mut out = Map::with_capacity(map.len());
            for (k, v_inner) in map {
                let is_secret = SECRET_FIELDS.iter().any(|&f| k.to_ascii_lowercase() == f);
                if is_secret {
                    // Redact entire subtree regardless of type (D1: {"password":["hunter2"]} was leaking).
                    *leaves += 1;
                    if *leaves > MAX_JSON_LEAVES {
                        return Err(IngestError::JsonTooComplex);
                    }
                    // Use typed category when value is a string and matches a known pattern (D3).
                    let cat = if let Value::String(ref s) = v_inner {
                        typed_category(s).unwrap_or(RedactedCategory::GenericSecret)
                    } else {
                        RedactedCategory::GenericSecret
                    };
                    r.categories.push(cat);
                    r.field_count += 1;
                    out.insert(k, Value::String(format!("[REDACTED:{cat:?}]")));
                    continue;
                }
                out.insert(k, walk(v_inner, depth + 1, leaves, r)?);
            }
            Ok(Value::Object(out))
        }
        Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(walk(item, depth + 1, leaves, r)?);
            }
            Ok(Value::Array(out))
        }
        other => Ok(other),
    }
}

pub fn redact_secrets(payload: Value) -> Result<(Value, RedactionReceipt), IngestError> {
    let mut receipt = RedactionReceipt {
        event_id: String::new(),
        categories: vec![],
        field_count: 0,
    };
    let scrubbed = walk(payload, 0, &mut 0, &mut receipt).map_err(|e| match e {
        IngestError::JsonTooDeep | IngestError::JsonTooComplex => e,
        other => IngestError::RedactionFailed(other.to_string()),
    })?;
    Ok((scrubbed, receipt))
}

#[cfg(test)]
mod tests;
