mod impl_;
mod receipt;
mod runner;
mod secret;
mod types;

pub use receipt::assemble_gate_evidence;
pub use runner::{
    evaluate_coverage, run_all_gates, CoverageProvider, FakeCoverageProvider, FakeGateRunner,
    GateRunner,
};
pub use secret::{FakeFindingsProvider, FindingsProvider, normalize_findings};
// Internal canonical types (new implementations — distinct from impl_'s GateSpec/GateResult)
pub use types::{
    GateEvidence, ReviewedCandidate, SecretFinding, Status, VerifyError,
};
pub use types::GateResult as CanonicalGateResult;
pub use types::GateSpec as CanonicalGateSpec;

// Re-exports from the inlined implementation (formerly fleet-verify).
pub use impl_::{
    FailReason, GateAssetError, GateCommand, GateResult, GateSpec, GatesRoot,
    ProbeTool, ProcessOutput, ProcessRunner, Report, Requirement, ToolProbe, Verdict,
    GATES, run_all, run_gate,
};

pub fn verify(
    candidate: &ReviewedCandidate,
    runner: &dyn GateRunner,
    findings_provider: &dyn FindingsProvider,
    coverage_provider: &dyn CoverageProvider,
) -> Result<GateEvidence, VerifyError> {
    if candidate.gates.is_empty() {
        return Err(VerifyError::NoGates);
    }
    let mut results = run_all_gates(&candidate.gates, &candidate.tree_digest, runner)?;
    if let Some(floor) = candidate.coverage_floor {
        let cov = evaluate_coverage(floor, &candidate.tree_digest, coverage_provider)?;
        results.push(cov);
    }
    let findings = findings_provider.scan(&candidate.tree_digest)?;
    let findings = normalize_findings(findings);
    Ok(assemble_gate_evidence(candidate, results, findings))
}

#[cfg(test)]
mod tests;
