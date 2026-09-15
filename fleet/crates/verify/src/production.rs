#[path = "production_acceptance.rs"]
mod acceptance;
#[path = "production_candidate.rs"]
mod candidate;
#[path = "production_output.rs"]
mod output;
#[path = "production_report.rs"]
mod report;
#[path = "production_runner.rs"]
mod runner;
#[cfg(test)]
#[path = "production_tests.rs"]
mod tests;
#[path = "production_tree.rs"]
mod tree;

pub use acceptance::acceptance_digest_for_specs;
pub use candidate::{
    candidate_for_repo, candidate_for_repo_with_specs, candidate_from_registry,
    candidate_from_resolved_specs, candidate_from_specs,
};
pub use report::report_from_evidence;
pub use runner::ProductionGateRunner;
pub use tree::tree_digest_for_repo;

use crate::impl_;
use crate::types::{ReviewedCandidate, VerifyError};
use crate::{CoverageProvider, FindingsProvider, GateEvidence};

pub fn verify_production<P, R, C>(
    candidate: &ReviewedCandidate,
    probe: &P,
    runner: &R,
    gates: &impl_::GatesRoot,
    specs: &[impl_::GateSpec],
    findings: &C,
) -> Result<GateEvidence, VerifyError>
where
    P: impl_::ToolProbe + Send + Sync,
    R: impl_::ProcessRunner + Send + Sync,
    C: FindingsProvider + Send + Sync,
{
    crate::verify(
        candidate,
        &runner::ProductionGateRunner::new(probe, runner, gates, specs),
        findings,
        &NoCoverage,
    )
}

struct NoCoverage;
impl CoverageProvider for NoCoverage {
    fn coverage_percent(&self, _: &str) -> Result<u64, VerifyError> {
        Err(VerifyError::InvalidCandidate(
            "coverage provider is not configured".into(),
        ))
    }
}
