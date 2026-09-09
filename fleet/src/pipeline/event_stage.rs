//! `Event`'s wiring, split out of `stages.rs` to stay under the 80-line file gate. Appends a real
//! `ReceiptEvent::RunStart` row to `fleet_store`'s hash-chained ledger under `state_dir` -- the
//! same `Ledger::append` path `fleet ledger` reads back -- instead of the literal `Ok(())` this
//! stage used to return, which left a pipeline run with no auditable trace at all.

use super::event::PipelineError;
use super::ledger_events;
use fleet_types::{ReceiptEvent, TaskId};
use std::path::Path;

pub fn event(state_dir: &Path, task: &TaskId) -> Result<(), PipelineError> {
    // `FileLock::acquire` opens `lock` with `create(true)` but never creates missing parent
    // directories; nothing upstream of `Event` (the pipeline's first stage) has created
    // `state_dir` yet on a fresh run, so this stage must.
    std::fs::create_dir_all(state_dir)
        .map_err(|e| PipelineError::Event(format!("could not create {}: {e}", state_dir.display())))?;
    let body = serde_json::json!({ "task_id": task.as_str() });
    ledger_events::append(state_dir, ReceiptEvent::RunStart, body).map_err(PipelineError::Event)
}
