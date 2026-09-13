//! Shared ledger-append plumbing for the pipeline's observability trail. `event_stage` uses
//! `open`/`append` directly (a ledger-append failure there is a real pipeline failure -- no
//! `RunStart` row means no auditable trace at all). Every OTHER call site in this module
//! (`observe`) treats a ledger-append failure as non-fatal: observability must never break a
//! run that would otherwise have succeeded (the append still surfaces on stderr, never silently
//! swallowed -- see `PRINCIPLES.md` on checks cheaper to fake than to satisfy).

use crate::print::human_stream::emit;
use crate::print::render_event::Event;
use crate::print::style::Style;
use serde_json::Value;
use std::path::Path;
use store::ledger::LedgerPaths;
use store::Ledger;
use types::ReceiptEvent;

/// The one place `LedgerPaths` is assembled from `state_dir` -- every stage that touches the
/// ledger goes through this instead of re-deriving `ledger.chain`/`ledger.lock` itself.
pub fn open(state_dir: &Path) -> Ledger {
    let paths = LedgerPaths {
        chain: state_dir.join("ledger.chain"),
        lock: state_dir.join("ledger.lock"),
    };
    Ledger::open(paths)
}

pub fn append(state_dir: &Path, event: ReceiptEvent, body: Value) -> Result<(), String> {
    append_as(state_dir, event, body, "fleet-cli-pipeline")
}

/// Append a parent-owned receipt for a non-pipeline user surface. The actor is explicit so an
/// interactive refusal cannot be misattributed to the pipeline worker path.
pub fn append_as(
    state_dir: &Path,
    event: ReceiptEvent,
    body: Value,
    actor: &str,
) -> Result<(), String> {
    open(state_dir)
        .append(event, body, actor.to_string(), None, None)
        .map(|_receipt| ())
        .map_err(|e| e.to_string())
}

/// Append a parent-owned receipt carrying the model reported by a provider response. Callers must
/// pass `None` for a requested alias; this field is evidence of what actually answered.
pub fn append_as_with_model(
    state_dir: &Path,
    event: ReceiptEvent,
    body: Value,
    actor: &str,
    resolved_model: Option<&str>,
) -> Result<(), String> {
    open(state_dir)
        .append(
            event,
            body,
            actor.to_string(),
            resolved_model.map(str::to_owned),
            None,
        )
        .map(|_receipt| ())
        .map_err(|e| e.to_string())
}

/// Best-effort append for the observability trail: stage timings, gate verdicts, refusals. A
/// failure here is reported through the same `Note`-styled render path a sink failure uses, then
/// ignored -- it must never turn an otherwise-successful pipeline run into a failed one.
pub fn observe(state_dir: &Path, event: ReceiptEvent, body: Value) {
    if let Err(reason) = append(state_dir, event, body) {
        let note = Event::Note {
            source: "ledger".to_string(),
            text: format!("append failed: {reason}"),
        };
        emit(&note, &Style::detect());
    }
}
