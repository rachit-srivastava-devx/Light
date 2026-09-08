//! `HumanApproval`. Ported from `fleet/keel/fleet/src/lifecycle.rs:169-186`.

use fleet_types::GateRefusal;

/// Proof that a human, rather than an agent, approved a gated edge (`review`, `accept`).
/// Only code in this crate can mint one -- the daemon's human-approval boundary in `src/`
/// calls this constructor; nothing downstream can forge approval by constructing the token
/// some other way.
#[derive(Debug)]
pub struct HumanApproval {
    pub(crate) evidence: String,
}

impl HumanApproval {
    /// `Err(GateRefusal{code: "EMPTY_HUMAN_APPROVAL", ..})` iff `evidence.trim()` is empty.
    pub fn recorded(evidence: impl Into<String>) -> Result<Self, GateRefusal> {
        let evidence = evidence.into();
        if evidence.trim().is_empty() {
            return Err(GateRefusal::new(
                "EMPTY_HUMAN_APPROVAL",
                "human approval evidence must not be empty",
            ));
        }
        Ok(Self { evidence })
    }
}
