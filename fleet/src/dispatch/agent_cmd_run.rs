//! Runs one adapter's real work for `__agent`. Ported in spirit from keel's
//! `agent_command`/`run_freelane_agent` (`git show HEAD:fleet/keel/fleet/src/main.rs`).
//! `Freelane` now shells out to the freelane.sh keyless lane restored into
//! `crates/fleet-worker/src/freelane/` (see that module for the embed/materialize/invoke split
//! this file only wires together); `Claude`/`Codex` invoke their CLI directly.
//!
//! Non-TTY invocation: `spawn::child_command::build` sets `stdin(Stdio::null())`, so the child
//! sees no TTY. `claude`'s default REPL refuses under those conditions -- v2.1.268 exits 1 with
//! empty stderr; `claude -p "<prompt>"` runs and prints the response. Verified 2026-09-11.
//! `codex`'s non-interactive shape is `codex exec "<prompt>"`. `run_cli` therefore branches on
//! adapter to pick the right non-REPL flag; Freelane never reaches `run_cli` (see `run` match).

use fleet_worker::freelane;
use fleet_worker::CliAdapter;
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};

/// What `run` produced: a genuine "done" body (+ the model that actually answered), or a genuine
/// "refuse" reason. Never fabricated.
pub enum AgentOutcome {
    Done { body: Value, resolved_model: Option<String> },
    Refused(String),
}

pub fn run(adapter: CliAdapter, worktree: &Path, task: &str, model: Option<&str>) -> AgentOutcome {
    match adapter {
        CliAdapter::Freelane => run_freelane(worktree, task, model),
        CliAdapter::Claude | CliAdapter::Codex => run_cli(adapter, worktree, task, model),
    }
}

/// `fleet_worker::freelane::run` owns asset resolution (embedded, or `$FLEET_FREELANE_ROOT` on
/// disk -- see that module) and invocation; this just shapes the fd-3 body from its typed result.
fn run_freelane(worktree: &Path, task: &str, model: Option<&str>) -> AgentOutcome {
    match freelane::run(worktree, task, model) {
        Ok(out) => {
            let resolved_model = out.resolved_model.clone();
            // Surface apply's signal: applied N files, vs named no target (`apply_note` set,
            // e.g. `AmbiguousTarget`), vs no code at all (both empty, e.g. `NoFence`).
            let applied: Vec<String> = out.applied_files.iter().map(|p| p.display().to_string()).collect();
            AgentOutcome::Done {
                body: json!({
                    "agent": "freelane",
                    "status": "done",
                    "response": out.response,
                    "log": out.log,
                    "resolved_model": out.resolved_model,
                    "tokens": out.tokens,
                    "applied_files": applied,
                    "apply_note": out.apply_note,
                }),
                resolved_model,
            }
        }
        Err(err) => AgentOutcome::Refused(err.to_string()),
    }
}

/// `claude`/`codex`: `fleet_worker::spawn` already confirmed the named binary is on `PATH`
/// before ever spawning this child, so invoke it for real with the task as its argument.
fn run_cli(adapter: CliAdapter, worktree: &Path, task: &str, model: Option<&str>) -> AgentOutcome {
    let binary = adapter.cli_binary_name().unwrap_or("true");
    let mut cmd = Command::new(binary);
    // Non-TTY branch per module doc: default REPL refuses under Stdio::null; each CLI needs its
    // non-interactive flag. Freelane never lands here (dispatched to run_freelane above), but
    // matched exhaustively so a future variant is a compile error, not a silent wrong flag.
    match adapter {
        CliAdapter::Claude => {
            cmd.arg("-p").arg(task);
        }
        CliAdapter::Codex => {
            cmd.arg("exec").arg(task);
        }
        CliAdapter::Freelane => {
            cmd.arg(task);
        }
    }
    let output = cmd.current_dir(worktree).stdin(Stdio::null()).output();
    match output {
        Ok(out) if out.status.success() => AgentOutcome::Done {
            body: json!({
                "agent": adapter.agent_kind(),
                "status": "done",
                "response": String::from_utf8_lossy(&out.stdout).trim().to_string(),
                "log": String::from_utf8_lossy(&out.stderr).trim().to_string(),
            }),
            resolved_model: model.map(str::to_string),
        },
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            AgentOutcome::Refused(format!("{}: worker exited {:?}: {stderr}", adapter.agent_kind(), out.status.code()))
        }
        Err(err) => AgentOutcome::Refused(format!("{}: cannot launch: {err}", adapter.agent_kind())),
    }
}
