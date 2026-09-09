//! `RunEnd`/`Refusal` ledger receipts for the pipeline's own start/stop, split out of `graph.rs`
//! to stay under the 80-line file gate. Both are best-effort (`ledger_events::observe`): a
//! failure to append here must never turn an otherwise-successful run into a failed one, and
//! must never mask a real stage failure behind a ledger-append failure instead.

use super::event::PipelineError;
use super::ledger_events;
use super::stage::PipelineStage;
use crate::print::human_stream::emit;
use crate::print::render_event::Event;
use crate::print::style::Style;
use fleet_types::{ReceiptEvent, TaskId};
use std::path::Path;

/// A stage failed for real: render it through the same `Refusal`-shaped line every other refusal
/// in this binary uses, and give `fleet-stream` a durable row for it -- previously a failing
/// stage was silent until the run's final one-line summary.
pub fn refusal(state_dir: &Path, stage: PipelineStage, err: &PipelineError) {
    let reason = err.to_string();
    emit(&Event::Refusal { source: stage.name().to_string(), reason: reason.clone() }, &Style::detect());
    let body = serde_json::json!({ "stage": stage.name(), "reason": reason });
    ledger_events::observe(state_dir, ReceiptEvent::Refusal, body);
}

/// The run's own close-out row, whatever the outcome -- the counterpart to `event_stage`'s
/// `RunStart`, so a durable tail of the ledger always has a matching bookend.
pub fn run_end(state_dir: &Path, task: &TaskId, final_stage: PipelineStage, ok: bool) {
    let body =
        serde_json::json!({ "task_id": task.as_str(), "final_stage": final_stage.name(), "ok": ok });
    ledger_events::observe(state_dir, ReceiptEvent::RunEnd, body);
}
