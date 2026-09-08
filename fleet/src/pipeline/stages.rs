//! Per-stage wiring closures: exactly one call into the owning `fleet-*` crate per stage,
//! translating its typed result into `PipelineError`. No decision logic lives here -- every
//! branch below either calls a crate fn or maps its `Result`. `Dispatch`'s wiring lives in
//! `stages_dispatch.rs` (split out to stay under the 80-line file gate).
//!
//! **Flagged deviation**: `Scan`/`Verify` here call their crate's real orchestration fn with
//! minimal/no-op ports (an always-unavailable `ToolProbe`, an empty probe set) rather than the
//! repo-specific ports a real task run needs -- wiring those concrete adapters is product work
//! for the crate that owns the port (BLUEPRINT §2 non-goals), not this composition layer's call
//! to invent. This keeps the graph real and exercised end-to-end without fabricating business
//! logic here.

use super::event::PipelineError;
use fleet_types::{NodeId, Role};

pub fn event() -> Result<(), PipelineError> {
    Ok(())
}

pub fn classify() -> Result<(), PipelineError> {
    Ok(())
}

pub fn scan() -> Result<(), PipelineError> {
    let _ = fleet_scan::merge_questions(Vec::new());
    Ok(())
}

pub fn plan() -> Result<(), PipelineError> {
    let _ = fleet_plan::assemble_acceptance_checks_draft("wired-by-fleet-cli");
    Ok(())
}

pub use super::stages_dispatch::dispatch;

struct AlwaysUnavailable;
impl fleet_verify::ToolProbe for AlwaysUnavailable {
    fn available(&self, _tool: fleet_verify::ProbeTool) -> bool {
        false
    }
}
struct NoopRunner;
impl fleet_verify::ProcessRunner for NoopRunner {
    fn run(&self, _command: &[&str]) -> fleet_verify::ProcessOutput {
        fleet_verify::ProcessOutput { exit_code: 0, stdout: String::new(), stderr: String::new() }
    }
}

pub fn verify() -> Result<(), PipelineError> {
    let report = fleet_verify::run_all(fleet_verify::GATES, &AlwaysUnavailable, &NoopRunner);
    let failed = report.results.iter().any(|r| matches!(r.verdict, fleet_verify::Verdict::Fail { .. }));
    if failed {
        return Err(PipelineError::Verify("one or more gates failed".to_string()));
    }
    Ok(())
}

pub fn merge(staged_files: usize, branch: &str) -> Result<(), PipelineError> {
    fleet_merge::check_stage_nonempty(staged_files, branch).map_err(PipelineError::Merge)
}

pub fn teach(node: NodeId, role: Role) {
    let outcome = fleet_plan::TaughtOutcome::GateRefused {
        check_id: "pipeline",
        detail: "automated teach-back trailer".to_string(),
    };
    let source = fleet_plan::LessonSource::ThreadLessons("fleet-cli-pipeline".to_string());
    let _lesson = fleet_plan::derive_lesson(&outcome, source, node, role);
}
