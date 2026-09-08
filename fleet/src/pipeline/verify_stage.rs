//! `Verify`'s wiring, split out of `stages.rs` to stay under the 80-line file gate. Runs the real
//! committed gate table (`fleet_verify::GATES`) against the same real `WhichProbe`/`RealRunner`
//! ports `fleet oracle`/`fleet gate` already use (`dispatch::verify_ports`) -- not the
//! always-unavailable probe + always-`exit 0` runner this stage used to hardcode, which made the
//! one stage whose job is catching failure structurally unable to ever fail.

use super::event::PipelineError;
use crate::dispatch::verify_ports::{resolve_gates_root, RealRunner, WhichProbe};
use fleet_verify::{GateSpec, Verdict};

pub fn verify(gates: &[GateSpec]) -> Result<(), PipelineError> {
    let gates_root =
        resolve_gates_root().map_err(|e| PipelineError::Verify(format!("gates root: {e}")))?;
    let report = fleet_verify::run_all(gates, &WhichProbe, &RealRunner::new(), &gates_root);
    let failed: Vec<String> = report
        .results
        .iter()
        .filter_map(|r| match &r.verdict {
            Verdict::Fail { reason, .. } => Some(format!("{}: {reason:?}", r.id)),
            _ => None,
        })
        .collect();
    if !failed.is_empty() {
        return Err(PipelineError::Verify(format!("gate(s) failed: {}", failed.join("; "))));
    }
    Ok(())
}
