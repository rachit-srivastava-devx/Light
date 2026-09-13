//! `FakeGateRunner`/`FakeCoverageProvider` -- split out of `runner.rs` for its 80-line cap.

use super::{CoverageProvider, GateRunner};
use crate::types::{GateResult, GateSpec, VerifyError};

pub struct FakeGateRunner {
    exit_code: i32,
}

impl FakeGateRunner {
    pub fn passing() -> Self {
        Self { exit_code: 0 }
    }
    pub fn failing() -> Self {
        Self { exit_code: 1 }
    }
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
            failure_message: if passed {
                None
            } else {
                Some(format!(
                    "gate '{}' failed (exit {})",
                    gate.id, self.exit_code
                ))
            },
        })
    }
}

pub struct FakeCoverageProvider {
    percent: u64,
}

impl FakeCoverageProvider {
    pub fn with(percent: u64) -> Self {
        Self { percent }
    }
}

impl CoverageProvider for FakeCoverageProvider {
    fn coverage_percent(&self, _tree_digest: &str) -> Result<u64, VerifyError> {
        Ok(self.percent)
    }
}
