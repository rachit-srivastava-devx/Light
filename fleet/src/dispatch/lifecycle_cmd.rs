//! `fleet lifecycle`: parse -> `fleet_lifecycle::{resume,advance_any}` -> print. Forwards to the
//! already-blueprinted `fleet-lifecycle`; this layer supplies only the `ReceiptLedger` IO port
//! (a plain append-only JSON-lines file under `state_dir`) that crate declares and does not
//! implement itself.

use crate::cli::args_ops::LifecycleArgs;
use crate::dispatch::error::DispatchError;
use crate::print::human;
use fleet_lifecycle::{advance_any, resume, ReceiptLedger, TaskId, TransitionReceipt};
use fleet_types::GateRefusal;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

struct FileReceiptLedger(PathBuf);

impl ReceiptLedger for FileReceiptLedger {
    fn append(&self, receipt: TransitionReceipt) -> Result<(), GateRefusal> {
        let line = format!("{} {} -> {} : {}\n", receipt.task_id, receipt.from, receipt.to, receipt.evidence);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.0)
            .map_err(|e| GateRefusal::new("RECEIPT_IO", e.to_string()))?;
        file.write_all(line.as_bytes()).map_err(|e| GateRefusal::new("RECEIPT_IO", e.to_string()))
    }
}

#[derive(serde::Serialize)]
struct LifecycleReport {
    advanced_to: String,
}

pub fn lifecycle(state_dir: &Path, args: LifecycleArgs) -> Result<(), DispatchError> {
    let ledger = FileReceiptLedger(state_dir.join("lifecycle-receipts.log"));
    let id = TaskId::new(args.task_id).map_err(DispatchError::Lifecycle)?;
    let any = resume("Intake", id, 0).map_err(DispatchError::Lifecycle)?;
    let advanced = advance_any(any, args.evidence, &ledger).map_err(DispatchError::Lifecycle)?;
    let advanced_to = format!("{advanced:?}");
    if args.json {
        crate::print::json::print_pretty(&LifecycleReport { advanced_to });
    } else {
        human::line("advanced_to", advanced_to);
    }
    Ok(())
}
