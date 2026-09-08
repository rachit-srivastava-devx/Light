//! `Task<Specified>`: `review`, `refuse`. Ported from
//! `fleet/keel/fleet/src/lifecycle.rs:222-238`.

use crate::human_approval::HumanApproval;
use crate::receipt::ReceiptLedger;
use crate::states::{Refused, Reviewed, Specified};
use crate::task::Task;
use fleet_types::GateRefusal;

impl Task<Specified> {
    /// Requires a `HumanApproval` -- an agent cannot self-review (`non_human_review`
    /// compile-fail test).
    pub fn review(
        self,
        approval: HumanApproval,
        ledger: &impl ReceiptLedger,
    ) -> Result<Task<Reviewed>, GateRefusal> {
        self.transition(approval.evidence, ledger)
    }

    pub fn refuse(
        self,
        reason: impl Into<String>,
        ledger: &impl ReceiptLedger,
    ) -> Result<Task<Refused>, GateRefusal> {
        self.transition(reason, ledger)
    }
}
