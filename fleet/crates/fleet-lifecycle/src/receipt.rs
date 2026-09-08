//! `TransitionReceipt`/`ReceiptLedger`. Ported from
//! `fleet/keel/fleet/src/lifecycle.rs:119-131`.

use crate::task_id::TaskId;
use fleet_types::GateRefusal;

/// A transition receipt awaiting the caller's own timestamp/actor stamping. `from`/`to` are
/// the `'static` type names of the marker structs (`std::any::type_name::<S>()`), never a
/// caller-suppliable string -- the receipt can only name a real compiled state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionReceipt {
    pub task_id: TaskId,
    pub from: &'static str,
    pub to: &'static str,
    pub evidence: String,
}

/// The only capability a transition needs from the caller's evidence ledger. The caller
/// (today `fleet-store`) implements this against a real hash-chained append-only file; this
/// crate never constructs a `ReceiptLedger` itself.
pub trait ReceiptLedger {
    fn append(&self, receipt: TransitionReceipt) -> Result<(), GateRefusal>;
}
