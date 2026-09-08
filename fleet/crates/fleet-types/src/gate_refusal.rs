//! `GateRefusal`. Lifted from `fleet/keel/fleet/src/lifecycle.rs:134-163` (there named
//! `Refusal`; disambiguated here from `fleet-router`'s distinct `RoleRefusal`).

use serde::{Deserialize, Serialize};

/// A failed gate check or a failed attempt to append a transition's evidence. `code` is always
/// a compiled-in `&'static str`; `message` is caller-supplied.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GateRefusal {
    code: &'static str,
    message: String,
}

impl GateRefusal {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}
