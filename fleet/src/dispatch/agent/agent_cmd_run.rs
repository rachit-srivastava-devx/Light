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

use builder::CliAdapter;
use builder::freelane;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};

#[path = "agent_stream.rs"]
mod agent_stream;
#[path = "agent_tool_policy.rs"]
mod agent_tool_policy;

pub use agent_tool_policy::AgentToolPolicy;

/// What `run` produced: a genuine "done" body (+ the model that actually answered), or a genuine
/// "refuse" reason. Never fabricated.
pub enum AgentOutcome {
    Done {
        body: Value,
        resolved_model: Option<String>,
        tokens: Option<u64>,
    },
    Refused(String),
}

pub fn run(adapter: CliAdapter, worktree: &Path, task: &str, model: Option<&str>) -> AgentOutcome {
    run_with_progress(
        adapter,
        worktree,
        task,
        model,
        AgentToolPolicy::worker_default(),
        |_| {},
    )
}

/// Exercises only the direct-CLI lane. Keep this test-only because production dispatch
/// intentionally routes Freelane through its embedded keyless provider instead.
#[cfg(test)]
fn run_cli(adapter: CliAdapter, worktree: &Path, task: &str, model: Option<&str>) -> AgentOutcome {
    run_cli_with_progress(
        adapter,
        worktree,
        task,
        model,
        AgentToolPolicy::worker_default(),
        |_| {},
    )
}

/// Runs one adapter and forwards provider text as it arrives. The interactive parent renders it;
/// the fd-3 child deliberately supplies a no-op because its only worker channel is fd 3.
pub fn run_with_progress<F>(
    adapter: CliAdapter,
    worktree: &Path,
    task: &str,
    model: Option<&str>,
    policy: AgentToolPolicy,
    progress: F,
) -> AgentOutcome
where
    F: FnMut(&str),
{
    match adapter {
        CliAdapter::Freelane => run_freelane(worktree, task, model),
        CliAdapter::Claude | CliAdapter::Codex => {
            run_cli_with_progress(adapter, worktree, task, model, policy, progress)
        }
    }
}

/// `builder::freelane::run` owns asset resolution (embedded, or `$FLEET_FREELANE_ROOT` on
/// disk -- see that module) and invocation; this just shapes the fd-3 body from its typed result.
fn run_freelane(worktree: &Path, task: &str, model: Option<&str>) -> AgentOutcome {
    match freelane::run(worktree, task, model) {
        Ok(out) => {
            let resolved_model = out.resolved_model.clone();
            // Surface apply's signal: applied N files, vs named no target (`apply_note` set,
            // e.g. `AmbiguousTarget`), vs no code at all (both empty, e.g. `NoFence`).
            let applied: Vec<String> = out
                .applied_files
                .iter()
                .map(|p| p.display().to_string())
                .collect();
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
                tokens: out.tokens,
            }
        }
        Err(err) => AgentOutcome::Refused(err.to_string()),
    }
}

/// `claude`/`codex`: `builder::spawn` already confirmed the named binary is on `PATH`
/// before ever spawning this child, so invoke it for real with the task as its argument.
fn run_cli_with_progress<F>(
    adapter: CliAdapter,
    worktree: &Path,
    task: &str,
    model: Option<&str>,
    policy: AgentToolPolicy,
    mut progress: F,
) -> AgentOutcome
where
    F: FnMut(&str),
{
    let Some(binary) = adapter.cli_binary_name() else {
        return AgentOutcome::Refused(format!(
            "{} has no direct CLI executable",
            adapter.agent_kind()
        ));
    };
    let mut cmd = Command::new(binary);
    // Claude CLI requires --print for non-interactive (batch) mode; without it, it tries
    // to start an interactive session and fails when stdin is null. `--print`/`--verbose`/
    // `--output-format stream-json` also drive the live tool-call milestones and the
    // restricted tool policy the interactive REPL depends on (see `agent_stream.rs`).
    if matches!(adapter, CliAdapter::Claude) {
        cmd.args(claude_stream_args());
        policy.configure_claude(&mut cmd);
    }
    // codex's non-interactive shape is `codex exec "<prompt>"` per the module doc above; a bare
    // positional arg leaves it waiting on an interactive session it will never get (stdin is
    // null here). Freelane never reaches this function (dispatched to `run_freelane` instead).
    if matches!(adapter, CliAdapter::Codex) {
        cmd.arg("exec");
    }
    cmd.arg(task);
    if let Some(m) = model {
        if matches!(adapter, CliAdapter::Claude) {
            cmd.args(["--model", m]);
        }
    }
    let launched = cmd
        .current_dir(worktree)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let Ok(mut child) = launched else {
        return AgentOutcome::Refused(format!("{}: cannot launch", adapter.agent_kind()));
    };
    let Some(stdout) = child.stdout.take() else {
        return AgentOutcome::Refused(format!("{}: stdout pipe unavailable", adapter.agent_kind()));
    };
    let Some(mut stderr) = child.stderr.take() else {
        return AgentOutcome::Refused(format!("{}: stderr pipe unavailable", adapter.agent_kind()));
    };
    let stderr_thread = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let mut raw_stdout = Vec::new();
    let mut stream = agent_stream::ClaudeStream::with_root(worktree);
    for line in BufReader::new(stdout).split(b'\n') {
        let Ok(line) = line else { break };
        raw_stdout.extend_from_slice(&line);
        raw_stdout.push(b'\n');
        let text = String::from_utf8_lossy(&line);
        if matches!(adapter, CliAdapter::Claude) {
            if let Some(delta) = stream.push(&text) {
                progress(&delta);
            }
        }
    }
    let waited = child.wait();
    let stderr = stderr_thread.join().unwrap_or_default();
    match waited {
        Ok(status)
            if status.success()
                && (!matches!(adapter, CliAdapter::Claude) || stream.has_result()) =>
        {
            let (body, resolved_model, tokens) = if matches!(adapter, CliAdapter::Claude) {
                let resolved_model = stream.model();
                let tokens = stream.tokens();
                (stream.body(), resolved_model, tokens)
            } else {
                (
                    json!({"response": String::from_utf8_lossy(&raw_stdout).trim(), "log": String::from_utf8_lossy(&stderr).trim()}),
                    None,
                    None,
                )
            };
            AgentOutcome::Done {
                body,
                resolved_model,
                tokens,
            }
        }
        Ok(status) => {
            let detail = failure_detail(&raw_stdout, &stderr);
            AgentOutcome::Refused(format!(
                "{}: worker exited {:?}: {detail}",
                adapter.agent_kind(),
                status.code()
            ))
        }
        Err(err) => AgentOutcome::Refused(format!("{}: cannot wait: {err}", adapter.agent_kind())),
    }
}

fn claude_stream_args() -> [&'static str; 5] {
    [
        "--print",
        "--verbose",
        "--output-format",
        "stream-json",
        "--include-partial-messages",
    ]
}

/// Providers do not agree on an error stream: Claude can put an API refusal on stdout.
/// Keep the non-empty diagnostic rather than reporting a useless trailing colon.
fn failure_detail(stdout: &[u8], stderr: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    if !stdout.is_empty() {
        return stdout;
    }
    "worker produced no diagnostic".into()
}

#[cfg(test)]
#[path = "agent_cmd_run_tests.rs"]
mod tests;
