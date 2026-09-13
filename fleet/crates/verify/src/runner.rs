use crate::types::{GateResult, GateSpec, VerifyError};

#[path = "runner_fakes.rs"]
mod runner_fakes;
pub use runner_fakes::{FakeCoverageProvider, FakeGateRunner};

pub trait GateRunner: Send + Sync {
    fn run(&self, gate: &GateSpec, tree_digest: &str) -> Result<GateResult, VerifyError>;
}

pub trait CoverageProvider: Send + Sync {
    fn coverage_percent(&self, tree_digest: &str) -> Result<u64, VerifyError>;
}

pub fn run_all_gates(
    gates: &[GateSpec],
    tree_digest: &str,
    runner: &dyn GateRunner,
) -> Result<Vec<GateResult>, VerifyError> {
    if gates.is_empty() {
        return Err(VerifyError::NoGates);
    }
    gates.iter().map(|g| runner.run(g, tree_digest)).collect()
}

pub fn evaluate_coverage(
    floor: u64,
    tree_digest: &str,
    provider: &dyn CoverageProvider,
) -> Result<GateResult, VerifyError> {
    let percent = provider.coverage_percent(tree_digest)?;
    let passed = percent >= floor;
    let failure_message = if passed {
        None
    } else {
        Some(format!("coverage {}% is below floor {}%", percent, floor))
    };
    Ok(GateResult {
        id: "coverage".to_string(),
        exit_code: if passed { 0 } else { 6 },
        stdout_digest: format!("coverage:{}", percent),
        stderr_digest: String::new(),
        input_digest: format!("floor:{}", floor),
        passed,
        failure_message,
    })
}
