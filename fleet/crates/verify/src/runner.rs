use crate::types::{GateResult, GateSpec, VerifyError};

pub trait GateRunner: Send + Sync {
    fn run(&self, gate: &GateSpec, tree_digest: &str) -> Result<GateResult, VerifyError>;
}

pub trait CoverageProvider: Send + Sync {
    fn coverage_percent(&self, tree_digest: &str) -> Result<f64, VerifyError>;
}

pub struct FakeGateRunner { exit_code: i32 }

impl FakeGateRunner {
    pub fn passing() -> Self { Self { exit_code: 0 } }
    pub fn failing() -> Self { Self { exit_code: 1 } }
}

impl GateRunner for FakeGateRunner {
    fn run(&self, gate: &GateSpec, _tree_digest: &str) -> Result<GateResult, VerifyError> {
        let passed = self.exit_code == 0;
        Ok(GateResult {
            id: gate.id.clone(),
            exit_code: self.exit_code,
            stdout_digest: "sha256:aabbcc".into(),
            stderr_digest: "sha256:ddeeff".into(),
            input_digest: "sha256:112233".into(),
            passed,
            failure_message: if passed { None } else {
                Some(format!("gate '{}' failed (exit {})", gate.id, self.exit_code))
            },
        })
    }
}

pub struct FakeCoverageProvider { percent: f64 }

impl FakeCoverageProvider {
    pub fn with(percent: f64) -> Self { Self { percent } }
}

impl CoverageProvider for FakeCoverageProvider {
    fn coverage_percent(&self, _tree_digest: &str) -> Result<f64, VerifyError> {
        Ok(self.percent)
    }
}

pub fn run_all_gates(
    gates: &[GateSpec],
    tree_digest: &str,
    runner: &dyn GateRunner,
) -> Result<Vec<GateResult>, VerifyError> {
    if gates.is_empty() { return Err(VerifyError::NoGates); }
    gates.iter().map(|g| runner.run(g, tree_digest)).collect()
}

pub fn evaluate_coverage(
    floor: u64,
    tree_digest: &str,
    provider: &dyn CoverageProvider,
) -> Result<GateResult, VerifyError> {
    let percent = provider.coverage_percent(tree_digest)?;
    let passed = percent >= floor as f64;
    let failure_message = if passed { None } else {
        Some(format!("coverage {:.1}% is below floor {}%", percent, floor))
    };
    Ok(GateResult {
        id: "coverage".to_string(),
        exit_code: if passed { 0 } else { 6 },
        stdout_digest: format!("coverage:{:.1}", percent),
        stderr_digest: String::new(),
        input_digest: format!("floor:{}", floor),
        passed,
        failure_message,
    })
}
