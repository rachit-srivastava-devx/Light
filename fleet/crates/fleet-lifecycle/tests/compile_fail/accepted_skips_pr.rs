// Accepted -> Observed must no longer exist. If this compiles, the PR-emit step is
// optional, and "the lifecycle ends at Observed" is still true.
use fleet_lifecycle::{ReceiptLedger, Task, TransitionReceipt};
use fleet_types::GateRefusal;

fn main() {
    let task: Task<fleet_lifecycle::Accepted> = unreachable!();
    let _ = task.observe("skipped the pull request", &NoLedger);
}

struct NoLedger;

impl ReceiptLedger for NoLedger {
    fn append(&self, _: TransitionReceipt) -> Result<(), GateRefusal> {
        Ok(())
    }
}
