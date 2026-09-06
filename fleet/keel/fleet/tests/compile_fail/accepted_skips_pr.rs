// Accepted -> Observed must no longer exist. If this compiles, the PR-emit step is
// optional, and "the lifecycle ends at Observed" (ORB-AND-FLEET-DELTA C6) is still true.
use fleet::lifecycle::{ReceiptLedger, Refusal, Task, TaskId, TransitionReceipt};

fn main() {
    let task: Task<fleet::lifecycle::Accepted> = unreachable!();
    let _ = task.observe("skipped the pull request", &NoLedger);
}

struct NoLedger;

impl ReceiptLedger for NoLedger {
    fn append(&self, _: TransitionReceipt) -> Result<(), Refusal> {
        Ok(())
    }
}
