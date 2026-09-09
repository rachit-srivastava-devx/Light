//! The two IO ports `fleet-verify` declares but does not implement itself: `ToolProbe` (is a
//! tool on `$PATH`) and `ProcessRunner` (actually spawn a gate's command line). Split out of
//! `verify_cmd.rs` purely to keep that file under the 80-line-per-file cap (BLUEPRINT §2/§7).
//! The bounded-execution logic (the S1 hang fix) lives in `verify_runner_bounded.rs`, split out
//! for the same reason.

use super::verify_runner_bounded::run_bounded;
use fleet_verify::{GateAssetError, GatesRoot, ProcessOutput, ProcessRunner, ProbeTool, ToolProbe};
use std::path::PathBuf;
use std::process::Command;
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

pub struct WhichProbe;
impl ToolProbe for WhichProbe {
    fn available(&self, tool: ProbeTool) -> bool {
        let name = match tool {
            ProbeTool::Cargo => "cargo",
            ProbeTool::CargoMutants => "cargo-mutants",
            ProbeTool::CargoFmt => "cargo-fmt",
            ProbeTool::CargoClippy => "cargo-clippy",
            ProbeTool::CargoDeny => "cargo-deny",
            ProbeTool::CargoAudit => "cargo-audit",
            ProbeTool::CargoLlvmCov => "cargo-llvm-cov",
            ProbeTool::Named(n) => n,
        };
        Command::new("which").arg(name).output().map(|o| o.status.success()).unwrap_or(false)
    }
}

/// Total wall-clock budget for one `oracle`/`gate`/pipeline-`Verify` invocation, shared across
/// every gate it runs. Fixes the S1 hang: `fleet_verify::GATES` includes `cargo test --workspace`
/// (measured ~94s on this machine) and `cargo mutants` (minutes-to-hours) run via a plain
/// `Command::output()` with no timeout, so `fleet run`/`fleet oracle`/`fleet gate` never returned.
/// Once the budget is spent, every remaining gate fails fast (typed `NonZeroExit(124)`, naming
/// itself in the `fleet: verify:` stderr line) instead of spawning. Override with
/// `FLEET_VERIFY_BUDGET_SECS` (tests use a short budget to stay fast and deterministic).
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
