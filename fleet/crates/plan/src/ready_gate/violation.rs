//! One shape violation: a dotted/bracket-indexed field path plus a human-readable reason.
//! Verbatim from `lld.rs:24-35`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub path: String,
    pub message: String,
}

pub(crate) fn v(path: impl Into<String>, message: impl Into<String>) -> Violation {
    Violation { path: path.into(), message: message.into() }
}
