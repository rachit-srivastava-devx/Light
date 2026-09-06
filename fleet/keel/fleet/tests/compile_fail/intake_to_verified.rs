use fleet::lifecycle::{Intake, Task, TaskId};

fn main() {
    let task: Task<Intake> = Task::new(TaskId::new("illegal").unwrap());
    let _ = task.verify("skipped every gate", &NoLedger);
}

struct NoLedger;

impl fleet::lifecycle::ReceiptLedger for NoLedger {
    fn append(
        &self,
        _: fleet::lifecycle::TransitionReceipt,
    ) -> Result<(), fleet::lifecycle::Refusal> {
        Ok(())
    }
}
