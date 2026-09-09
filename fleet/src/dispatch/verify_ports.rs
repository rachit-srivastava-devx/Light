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

/// Total wall-clock budget for one `oracle`/`gate`/pipeline-`Verify` invocation, shared across
/// every gate it runs. Fixes the S1 hang: a plain `Command::output()` with no timeout let
/// `fleet run`/`fleet oracle`/`fleet gate` never return. Once spent, every remaining gate fails
/// fast (typed `NonZeroExit(124)`) instead of spawning. Override with `FLEET_VERIFY_BUDGET_SECS`
/// (tests use a short budget to stay fast and deterministic).
pub fn verify_budget() -> Duration {
    std::env::var("FLEET_VERIFY_BUDGET_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(8))
}

/// Runs every gate's argv with `repo` as its cwd (the S1 fix) -- `OnPath` gates (`cargo test`)
/// and `Script` gates alike, so a gate script resolved from the gates-root still executes
/// against the target repo, never the calling process's own cwd.
pub struct RealRunner {
    deadline: Instant,
    repo: PathBuf,
}

impl RealRunner {
    pub fn new(repo: impl Into<PathBuf>) -> Self {
        Self { deadline: Instant::now() + verify_budget(), repo: repo.into() }
    }
}

impl ProcessRunner for RealRunner {
    fn run(&self, command: &[&str]) -> ProcessOutput {
        run_bounded(command, self.deadline, &self.repo)
    }
}
