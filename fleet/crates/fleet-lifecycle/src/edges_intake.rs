//! `Task<Intake>`: `new`, `specify`, `refuse`. Ported from
//! `fleet/keel/fleet/src/lifecycle.rs:196-220`.

use crate::receipt::ReceiptLedger;
use crate::states::{Intake, Refused, Specified};
use crate::task::Task;
use crate::task_id::TaskId;
use fleet_types::GateRefusal;
use std::marker::PhantomData;

impl Task<Intake> {
    pub fn new(id: TaskId) -> Self {
        Self { id, retry_depth: 0, _s: PhantomData }
    }

    pub fn specify(
        self,
        evidence: impl Into<String>,
        ledger: &impl ReceiptLedger,
    ) -> Result<Task<Specified>, GateRefusal> {
        self.transition(evidence, ledger)
    }

    pub fn refuse(
        self,
        reason: impl Into<String>,
        ledger: &impl ReceiptLedger,
    ) -> Result<Task<Refused>, GateRefusal> {
        self.transition(reason, ledger)
    }
}
