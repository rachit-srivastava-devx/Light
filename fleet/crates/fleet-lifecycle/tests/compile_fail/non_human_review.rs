use fleet_lifecycle::{ReceiptLedger, Task, TaskId, TransitionReceipt};
use fleet_types::GateRefusal;

fn main() {
    let intake = Task::new(TaskId::new("agent-review").unwrap());
    let specified = intake.specify("SOW", &NoLedger).unwrap();
    let _reviewed = specified.review("agent says yes", &NoLedger);
}

struct NoLedger;

impl ReceiptLedger for NoLedger {
    fn append(&self, _: TransitionReceipt) -> Result<(), GateRefusal> {
        Ok(())
    }
}
