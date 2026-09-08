//! Runs one adapter's real work for `__agent`. Ported in spirit from keel's
//! `agent_command`/`run_freelane_agent` (`git show HEAD:fleet/keel/fleet/src/main.rs`).
//! `Freelane` now shells out to the freelane.sh keyless lane restored into
//! `crates/fleet-worker/src/freelane/` (see that module for the embed/materialize/invoke split
//! this file only wires together); `Claude`/`Codex` invoke their CLI directly.

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
            AgentOutcome::Done {
                body: json!({
                    "agent": "freelane",
                    "status": "done",
                    "response": out.response,
                    "log": out.log,
                    "resolved_model": out.resolved_model,
                    "tokens": out.tokens,
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
    let output = Command::new(binary)
        .arg(task)
        .current_dir(worktree)
        .stdin(Stdio::null())
        .output();
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
        Ok(out) => AgentOutcome::Refused(format!(
            "{}: worker exited {:?}: {}",
            adapter.agent_kind(),
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(err) => AgentOutcome::Refused(format!(
            "{}: cannot launch: {err}",
            adapter.agent_kind()
        )),
    }
}
