//! Args for the hidden `__agent` child-side entry point. Positional shape must match
//! `fleet-worker`'s `child_command::build` EXACTLY: `<kind> <worktree> <task> [model]`. Any
//! drift here re-opens the bug this file exists to close (see `dispatch/agent_cmd.rs`).

use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct AgentArgs {
    /// Adapter kind token, the inverse of `CliAdapter::agent_kind()` (`"claude"`, `"codex"`,
    /// `"freelane"`). Unknown tokens are a typed error, never a silent default.
    pub kind: String,
    /// The worktree the child is expected to do its work in.
    pub worktree: PathBuf,
    /// Free-text task prompt.
    pub task: String,
    /// Optional requested model, only meaningful for `claude`/`codex`.
    pub model: Option<String>,
}

/// `fleet __spawn_probe`: drives `fleet_worker::spawn`/`join`'s REAL parent-side path (real
/// worktree, real `socketpair`, a real re-exec of THIS binary as `__agent`) end to end through
/// the compiled binary -- the only way to test that path without either running inside the test
/// harness process (whose `current_exe()` is the harness, not `fleet`) or reaching for the
/// fault-injection-only `FLEET_WORKER_TEST_CHILD_EXE` seam. See `dispatch/spawn_probe_cmd.rs`.
#[derive(Args, Debug)]
pub struct SpawnProbeArgs {
    #[arg(long)]
    pub repo: String,
}
