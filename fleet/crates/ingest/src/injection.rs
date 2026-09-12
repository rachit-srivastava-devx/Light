use std::sync::OnceLock;
use regex::Regex;
use serde_json::Value;
use crate::types::MAX_JSON_DEPTH;

static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

fn patterns() -> &'static [Regex] {
    PATTERNS.get_or_init(|| vec![
        Regex::new(r"(?i)ignore\s+(all\s+)?previous\s+instructions").unwrap(),
        Regex::new(r"(?i)you\s+are\s+now\s+in\s+\S+\s+mode").unwrap(),
        Regex::new(r"(?i)(reveal|show|print|return)\s+(your\s+)?system\s+prompt").unwrap(),
        Regex::new(r"(?i)(perform|use|attempt)\s+a\s+jailbreak").unwrap(),
        Regex::new(r"(?i)disregard\s+(all\s+)?(previous\s+)?instructions").unwrap(),
    ])
}

fn scan(v: &Value, depth: usize) -> bool {
    if depth > MAX_JSON_DEPTH { return false; }
    match v {
        Value::String(s) => patterns().iter().any(|p| p.is_match(s)),
        Value::Object(m) => m.values().any(|c| scan(c, depth + 1)),
        Value::Array(a) => a.iter().any(|c| scan(c, depth + 1)),
        _ => false,
    }
}

/// Returns `true` if the payload contains prompt-injection patterns.
/// Never refuses — callers set `injection_taint` on the envelope.
pub fn check_injection(payload: &Value) -> bool {
    scan(payload, 0)
}
