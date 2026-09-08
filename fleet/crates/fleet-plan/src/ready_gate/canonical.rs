//! Canonical JSON + content hash. Verbatim from `lld.rs:757-827`.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// A JSON number was encountered while canonicalizing -- refused structurally (three-language
/// float-formatting divergence, `lld.rs:762-767`), never silently coerced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericLeafError;

impl std::fmt::Display for NumericLeafError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "numbers are not canonicalizable (F02 §6.3)")
    }
}

impl std::error::Error for NumericLeafError {}

/// Deterministic JSON serialisation: object keys sorted recursively, array order preserved.
pub fn canonical_json(value: &Value) -> Result<String, NumericLeafError> {
    match value {
        Value::Number(_) => Err(NumericLeafError),
        Value::Array(items) => {
            let mut parts = Vec::with_capacity(items.len());
            for item in items {
                parts.push(canonical_json(item)?);
            }
            Ok(format!("[{}]", parts.join(",")))
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut parts = Vec::with_capacity(keys.len());
            for k in keys {
                let key_json = serde_json::to_string(k).expect("string keys always serialize");
                parts.push(format!("{key_json}:{}", canonical_json(&map[k])?));
            }
            Ok(format!("{{{}}}", parts.join(",")))
        }
        other => Ok(serde_json::to_string(other).expect("non-numeric scalars always serialize")),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

pub(crate) fn hash_canonical_string(canonical: &str) -> String {
    format!("sha256:{}", hex_encode(&Sha256::digest(canonical.as_bytes())))
}

/// `"sha256:" + hex(sha256(canonical_json(value)))`.
pub fn content_hash(value: &Value) -> Result<String, NumericLeafError> {
    Ok(hash_canonical_string(&canonical_json(value)?))
}
