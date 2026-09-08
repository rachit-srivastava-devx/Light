use fleet_lifecycle::{Intake, ReceiptLedger, Task, TaskId, TransitionReceipt};
use fleet_types::GateRefusal;

fn main() {
    let task: Task<Intake> = Task::new(TaskId::new("moved").unwrap());
    let _specified = task.specify("first transition", &NoLedger).unwrap();
    let _again = task.specify("second transition", &NoLedger);
}

struct NoLedger;

impl ReceiptLedger for NoLedger {
    fn append(&self, _: TransitionReceipt) -> Result<(), GateRefusal> {
        Ok(())
    }
}
