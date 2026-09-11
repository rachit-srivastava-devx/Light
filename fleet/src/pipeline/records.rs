//! What one `fleet run` actually did, as data the `--json` path can carry.
//!
//! `--json` used to emit three keys (`task`, `final_stage`, `classification`) while the human
//! stderr path printed every stage's outcome and every gate's verdict -- the machine path was
//! strictly LESS informative than the human one, which defeats its purpose. These records are
//! filled by the same call sites that emit the human events, so the two cannot drift.
//! `run_records.rs` holds the accumulator that gathers them.

use super::stage::PipelineStage;
use crate::print::render_event::Outcome;

/// One stage's outcome. `outcome` is `"pass"`, `"fail"`, or `"resumed"` -- the third is a stage
/// the step log had already marked done, so this invocation ran nothing for it.
#[derive(Debug, serde::Serialize)]
pub struct StageRecord {
    pub stage: PipelineStage,
    pub outcome: &'static str,
    /// Absent (not `0`) for a resumed stage: this invocation spent no time in it, and a
    /// fabricated zero would read as "ran, instantly" (hard rule 9 -- absent is absent).
    pub elapsed_ms: Option<u128>,
}

/// One gate's verdict, mirroring the `GateVerdict` event the human path renders.
#[derive(Debug, serde::Serialize)]
pub struct GateRecord {
    pub id: String,
    /// `"pass" | "fail" | "skip"`.
    pub verdict: &'static str,
    /// The published denominator, both halves or neither -- never one alone.
    pub checked: Option<u64>,
    pub total: Option<u64>,
    /// The `FailReason` for a failure, the skip reason for a skip.
    pub detail: Option<String>,
    /// A REQUIRED gate that skipped: an environment fault (exit code 3), not a pass. Surfaced
    /// per-gate so a caller can tell "8 gates, none failed" from "8 gates, 5 never ran".
    pub env_fault: bool,
}

pub fn label(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Pass => "pass",
        Outcome::Fail => "fail",
        Outcome::Skip => "skip",
        Outcome::Progress => "running",
    }
}
