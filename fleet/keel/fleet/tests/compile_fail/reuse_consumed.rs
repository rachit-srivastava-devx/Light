use fleet::lifecycle::{Intake, ReceiptLedger, Refusal, Task, TaskId, TransitionReceipt};

fn main() {
    let task: Task<Intake> = Task::new(TaskId::new("moved").unwrap());
    let _specified = task.specify("first transition", &NoLedger).unwrap();
    let _again = task.specify("second transition", &NoLedger);
}

struct NoLedger;

impl ReceiptLedger for NoLedger {
    fn append(&self, _: TransitionReceipt) -> Result<(), Refusal> {
        Ok(())
    }
}
