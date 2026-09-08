//! `Task<Attested>::accept`. Ported from `fleet/keel/fleet/src/lifecycle.rs:286-294`.

use crate::human_approval::HumanApproval;
use crate::receipt::ReceiptLedger;
use crate::states::{Accepted, Attested};
use crate::task::Task;
use fleet_types::GateRefusal;

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
