//! The two IO ports `fleet-verify` declares but does not implement itself: `ToolProbe` (is a
//! tool on `$PATH`) and `ProcessRunner` (actually spawn a gate's command line). Split out of
//! `verify_cmd.rs` to keep that file under the 80-line-per-file cap (BLUEPRINT §2/§7); the
//! bounded-execution logic (S1 hang fix) lives in `verify_runner_bounded.rs` for the same reason.

use super::verify_runner_bounded::run_bounded;
use fleet_verify::{GateAssetError, GatesRoot, ProcessOutput, ProcessRunner};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Where `GateCommand::Script` gates resolve their scripts from. `$FLEET_GATES_ROOT`, if set,
/// overrides the embedded copies with real files on disk (fleet-verify's own injected-port style,
/// applied at this CLI wiring layer -- see `crates/fleet-verify/src/gates/mod.rs`); otherwise the
/// scripts baked into the binary at compile time are materialized fresh, so this works regardless
/// of the process's cwd.
pub fn resolve_gates_root() -> Result<GatesRoot, GateAssetError> {
    match std::env::var_os("FLEET_GATES_ROOT") {
        Some(path) => GatesRoot::from_override(path),
        None => GatesRoot::materialize(),
    }
}

/// TOTAL wall-clock budget for one `oracle`/`gate`/pipeline-`Verify` invocation. Fixes the S1
/// hang (no timeout meant these commands never returned). Override with
/// `FLEET_VERIFY_BUDGET_SECS`. Default was 8s, which no real `cargo test` on a cold repo can
/// meet -- so `fleet run` spent it all in gate #1 and reported timeout 124 for the other seven.
pub fn verify_budget() -> Duration {
    std::env::var("FLEET_VERIFY_BUDGET_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(default_total_budget)
}

/// The no-env default, pure so a test can pin it without mutating process-global env.
pub fn default_total_budget() -> Duration {
    Duration::from_secs(300)
}

/// Per-gate ceiling, so ONE slow gate cannot starve the others. With a single shared deadline the
/// gate table's ORDER decided who got any time at all, and the report then blamed the starved
/// gates instead of the greedy one. Half the total: a slow gate still gets a generous slice, and
/// whatever it leaves unused stays available to the rest.
pub fn per_gate_budget(total: Duration) -> Duration {
    total / 2
}

/// Runs every gate's argv with `repo` as its cwd (the S1 fix) -- `OnPath` gates (`cargo test`)
/// and `Script` gates alike, so a gate script resolved from the gates-root still executes
/// against the target repo, never the calling process's own cwd.
pub struct RealRunner {
    /// Hard stop for the whole invocation -- the run always terminates by here.
    overall: Instant,
    /// Ceiling applied to each gate individually (see `per_gate_budget`).
    per_gate: Duration,
    repo: PathBuf,
}

impl RealRunner {
    pub fn new(repo: impl Into<PathBuf>) -> Self {
        let total = verify_budget();
        Self {
            overall: Instant::now() + total,
            per_gate: per_gate_budget(total),
            repo: repo.into(),
        }
    }
}

impl ProcessRunner for RealRunner {
    fn run(&self, command: &[&str]) -> ProcessOutput {
        // The earlier of the two: a gate may not outlive its own slice, nor the whole invocation.
        let deadline = (Instant::now() + self.per_gate).min(self.overall);
        run_bounded(command, deadline, &self.repo)
    }
}

#[cfg(test)]
#[path = "verify_ports_tests.rs"]
mod tests;
