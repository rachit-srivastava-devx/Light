use fleet_lifecycle::{Intake, Task, TaskId};

fn main() {
    let task: Task<Intake> = Task::new(TaskId::new("illegal").unwrap());
    let _ = task.verify("skipped every gate", &NoLedger);
}

struct NoLedger;

impl fleet_lifecycle::ReceiptLedger for NoLedger {
    fn append(
        &self,
        _: fleet_lifecycle::TransitionReceipt,
    ) -> Result<(), fleet_types::GateRefusal> {
        Ok(())
    }
}
