//! `Verify`'s wiring, split out of `stages.rs` to stay under the 80-line file gate. Runs the real
//! committed gate table (`fleet_verify::GATES`) against the same real `WhichProbe`/`RealRunner`
//! ports `fleet oracle`/`fleet gate` already use (`dispatch::verify_ports`) -- not the
//! always-unavailable probe + always-`exit 0` runner this stage used to hardcode, which made the
//! one stage whose job is catching failure structurally unable to ever fail.

use super::event::PipelineError;
use super::ledger_events;
use crate::dispatch::verify_ports::{resolve_gates_root, RealRunner, WhichProbe};
use crate::print::human_stream::emit;
use crate::print::render_event::Event;
use crate::print::style::Style;
use crate::print::verify_report::line_for;
use fleet_types::ReceiptEvent;
use fleet_verify::{GateSpec, Verdict};
use std::path::Path;

/// `repo` is the same `--repo` the pipeline was invoked with (`StageCtx::repo`) -- the S1 fix:
/// this stage used to run every gate against the `fleet` process's own cwd instead of the repo
/// the caller actually named.
pub fn verify(gates: &[GateSpec], repo: &Path, state_dir: &Path) -> Result<(), PipelineError> {
    let gates_root =
        resolve_gates_root().map_err(|e| PipelineError::Verify(format!("gates root: {e}")))?;
    let runner = RealRunner::new(repo);
    let report = fleet_verify::run_all(gates, &WhichProbe, &runner, &gates_root);
    let mut failed: Vec<String> = Vec::new();
    for r in &report.results {
        report_gate(state_dir, r);
        if let Verdict::Fail { reason, .. } = &r.verdict {
            failed.push(format!("{}: {reason:?}", r.id));
        }
    }
    if !failed.is_empty() {
        return Err(PipelineError::Verify(format!("gate(s) failed: {}", failed.join("; "))));
    }
    Ok(())
}

/// One gate's real, structured verdict, pushed to both consumers of the same `Event`: the human
/// render path (previously silent per-gate during `fleet run` -- only the stage's own pass/fail
/// showed) and the durable ledger `fleet-stream` tails.
fn report_gate(state_dir: &Path, r: &fleet_verify::GateResult) {
    let event = line_for(r);
    emit(&event, &Style::detect());
    let Event::GateVerdict { id, outcome, checked, total, detail } = &event else { return };
    let body = serde_json::json!({
        "id": id, "outcome": format!("{outcome:?}"), "checked": checked, "total": total, "detail": detail,
    });
    ledger_events::observe(state_dir, ReceiptEvent::GateVerdict, body);
}
