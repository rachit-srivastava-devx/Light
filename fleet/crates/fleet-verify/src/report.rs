//! The full run's outcome -- aggregates every `GateResult` into typed pass/fail/skip counters
//! and one overall `ExitCode`.

use fleet_types::ExitCode;

use crate::verdict::{GateResult, Verdict};

#[derive(Clone, Debug)]
pub struct Report {
    pub results: Vec<GateResult>,
}

impl Report {
    pub fn passed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r.verdict, Verdict::Pass(_)))
            .count()
    }

    pub fn failed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r.verdict, Verdict::Fail { .. }))
            .count()
    }

    pub fn skipped(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r.verdict, Verdict::Skip { .. }))
            .count()
    }

    /// Count of `Skip{was_required: true, ..}` -- `verify.sh`'s `ENV_FAIL` counter.
    pub fn env_faults(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r.verdict, Verdict::Skip { was_required: true, .. }))
            .count()
    }

    /// `ExitCode::Env` if `env_faults() > 0`; else `ExitCode::Invariant` if `failed() > 0`; else
    /// `ExitCode::Ok`. Env-fault check strictly outranks invariant-fail, matching
    /// `verify.sh:125-126`'s literal ordering.
    pub fn exit_code(&self) -> ExitCode {
        if self.env_faults() > 0 {
            ExitCode::Env
        } else if self.failed() > 0 {
            ExitCode::Invariant
        } else {
            ExitCode::Ok
        }
    }
}
