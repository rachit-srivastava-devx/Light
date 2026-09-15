mod candidate;
mod impl_;
mod production;
mod receipt;
mod runner;
mod secret;
mod types;

pub use production::{
    acceptance_digest_for_specs, candidate_for_repo, candidate_for_repo_with_specs,
    candidate_from_registry, candidate_from_resolved_specs, candidate_from_specs,
    report_from_evidence, tree_digest_for_repo, verify_production, ProductionGateRunner,
};
pub use receipt::{assemble_gate_evidence, secret_scan_integrity_digest};
pub use runner::{
    evaluate_coverage, run_all_gates, CoverageProvider, FakeCoverageProvider, FakeGateRunner,
    GateRunner,
};
pub use secret::{
    findings_summary, normalize_findings, scan_secrets, scan_secrets_with_mode,
    FakeFindingsProvider, FindingsProvider, GitleaksFindingsProvider, SecretScanScope,
};
// Internal canonical types (new implementations — distinct from impl_'s GateSpec/GateResult)
use candidate::validate_candidate;
pub use types::GateResult as CanonicalGateResult;
pub use types::GateSpec as CanonicalGateSpec;
pub use types::{GateEvidence, ReviewedCandidate, SecretFinding, Status, VerifyError};

// Re-exports from the inlined implementation (formerly fleet-verify).
pub use impl_::{
    run_all, run_gate, FailReason, GateAssetError, GateCommand, GateResult, GateSpec, GatesRoot,
    ProbeTool, ProcessOutput, ProcessRunner, Report, Requirement, ToolProbe, Verdict, GATES,
};

// Parsers are `reviewed data` (per the fleet-verify doctrine), NOT a public API surface for
// reuse. This type is re-exported (doc-hidden) SOLELY so integration tests under `tests/` can
// pin the exact `DenominatorResult` each committed gate's parser produces from real gate-script
// stdout; look the parser up via `GATES.iter().find(|g| g.id == …).parse_denominator` rather
// than importing a parser fn directly.
#[doc(hidden)]
pub use impl_::DenominatorResult;

pub fn verify(
    candidate: &ReviewedCandidate,
    runner: &dyn GateRunner,
    findings_provider: &dyn FindingsProvider,
    coverage_provider: &dyn CoverageProvider,
) -> Result<GateEvidence, VerifyError> {
    validate_candidate(candidate)?;
    let mut results = run_all_gates(&candidate.gates, &candidate.tree_digest, runner)?;
    for (spec, result) in candidate.gates.iter().zip(&results) {
        if result.id != spec.id {
            return Err(VerifyError::InvalidGate(format!(
                "runner returned '{}' for '{}'",
                result.id, spec.id
            )));
        }
    }
    if let Some(floor) = candidate.coverage_floor {
        let cov = evaluate_coverage(floor, &candidate.tree_digest, coverage_provider)?;
        results.push(cov);
    }
    let findings = findings_provider.scan(&candidate.tree_digest)?;
    let findings = normalize_findings(findings);
    Ok(assemble_gate_evidence(candidate, results, findings))
}

#[cfg(test)]
mod integrity_tests;
#[cfg(test)]
mod matrix_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod validation_tests;
