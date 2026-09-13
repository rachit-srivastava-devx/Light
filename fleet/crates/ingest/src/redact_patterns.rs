//! Secret-detection patterns/field names for `redact.rs::walk` -- split out for its 80-line cap.

use crate::types::RedactedCategory;
use regex::Regex;
use std::sync::OnceLock;

static PATTERNS: OnceLock<Vec<(Regex, RedactedCategory)>> = OnceLock::new();

pub(super) fn patterns() -> &'static [(Regex, RedactedCategory)] {
    PATTERNS.get_or_init(|| {
        vec![
            (
                Regex::new(r"sk-[A-Za-z0-9]{20,}").unwrap(),
                RedactedCategory::ApiKey,
            ),
            (
                Regex::new(r"Bearer [A-Za-z0-9\-._~+/]+=*").unwrap(),
                RedactedCategory::BearerToken,
            ),
            (
                Regex::new(r"-----BEGIN .{1,30} PRIVATE KEY-----").unwrap(),
                RedactedCategory::PrivateKey,
            ),
        ]
    })
}

pub(super) const SECRET_FIELDS: &[&str] = &[
    "api_key",
    "secret",
    "token",
    "password",
    "private_key",
    "authorization",
];

/// Match value against typed patterns; returns `None` if no pattern matches.
pub(super) fn typed_category(s: &str) -> Option<RedactedCategory> {
    patterns()
        .iter()
        .find(|(p, _)| p.is_match(s))
        .map(|(_, c)| *c)
}
