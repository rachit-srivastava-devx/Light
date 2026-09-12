//! `Task<Attested>::accept`. Ported from `fleet/keel/fleet/src/lifecycle.rs:286-294`.

use super::human_approval::HumanApproval;
use super::receipt::ReceiptLedger;
use super::states::{Accepted, Attested};
use super::task::Task;
use ::types::GateRefusal;

impl Task<Attested> {
    /// Requires a `HumanApproval` (`accept` is a human-only gate, symmetric with `review`).
    pub fn accept(
        self,
        approval: HumanApproval,
        ledger: &impl ReceiptLedger,
    ) -> Result<Task<Accepted>, GateRefusal> {
        self.transition(approval.evidence, ledger)
    }
}
