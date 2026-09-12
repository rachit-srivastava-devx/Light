//! Runs one adapter's real work for `__agent`. Ported in spirit from keel's
//! `agent_command`/`run_freelane_agent` (`git show HEAD:fleet/keel/fleet/src/main.rs`).
//! `Freelane` now shells out to the freelane.sh keyless lane restored into
//! `crates/fleet-worker/src/freelane/` (see that module for the embed/materialize/invoke split
//! this file only wires together); `Claude`/`Codex` invoke their CLI directly.
//!
//! ## Claude adapter shape — empirical, not aesthetic
//!
//! The `Claude` arm invokes `claude -p <task>` **plus** four flags that turn Claude from a
//! one-shot prose generator into a tool-loop coder:
//!
//! - `--permission-mode acceptEdits` — Claude can Edit/Write without a prompt.
//! - `--allowedTools "Read Edit Write Bash Grep Glob"` — the file-and-shell tool set.
//! - `--append-system-prompt <CLAUDE_SYSTEM_PROMPT>` — the tuned prompt from
//!   `fleet-worker/assets/claude-system-prompt.md` (five named idioms + Fleet discipline).
//! - `--add-dir <worktree>` — grant Claude write access to the lane's worktree explicitly.
//!
//! Measured on Frido CSAT 2026-09-12 (iteration 2): the bare `claude -p "<task>"` shape wired
//! by PR #9 produced prose, not code — no tool loop, no file edits, no tests. The rich shape
//! above produced **Grade A−**: 37 passing tests, all 4 client requirements satisfied, and both
//! P0s the adversarial reviewer flagged on PR #148 (React `useState`-initializer-once + explicit
//! `Set<>` dedup) caught in the first pass, along with the Safari `revokeObjectURL` race, the
//! CSV formula-injection guard, and a stable sort tiebreak. FD-10 (hermetic env allowlist) still
//! gates end-to-end runs; the adapter shape is ready for the moment it unblocks.

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

/// Compose the `Command` that would be spawned for `adapter` (before setting stdio). Exposed
/// separately so tests can assert the exact arg list without shelling out to the real CLI.
///
/// - `Claude` gets the rich shape documented at the top of this file.
/// - `Codex` gets `codex exec <task>` — the shape keel used, unchanged by this PR.
/// - `Freelane` never reaches here (see `run`), but is included so a regression that routes it
///   through `run_cli` produces a bare `<task>` command rather than picking up Claude's flags.
pub(crate) fn build_cmd(adapter: CliAdapter, worktree: &Path, task: &str) -> Command {
    let binary = adapter.cli_binary_name().unwrap_or("true");
    let mut cmd = Command::new(binary);
    match adapter {
        CliAdapter::Claude => {
            cmd.arg("-p")
                .arg(task)
                .arg("--permission-mode")
                .arg("acceptEdits")
                .arg("--allowedTools")
                .arg("Read Edit Write Bash Grep Glob")
                .arg("--append-system-prompt")
                .arg(fleet_worker::CLAUDE_SYSTEM_PROMPT)
                .arg("--add-dir")
                .arg(worktree);
        }
        CliAdapter::Codex => {
            cmd.arg("exec").arg(task);
        }
        CliAdapter::Freelane => {
            cmd.arg(task);
        }
    }
    cmd.current_dir(worktree);
    cmd
}

/// `claude`/`codex`: `fleet_worker::spawn` already confirmed the named binary is on `PATH`
/// before ever spawning this child, so invoke it for real with the adapter-specific arg shape
/// composed by `build_cmd`. See the module doc for why Claude's shape is what it is.
fn run_cli(adapter: CliAdapter, worktree: &Path, task: &str, model: Option<&str>) -> AgentOutcome {
    let mut cmd = build_cmd(adapter, worktree, task);
    let output = cmd.stdin(Stdio::null()).output();
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

#[cfg(test)]
mod tests {
    //! These tests DO NOT spawn `claude` or `codex`. They inspect the `Command` value that
    //! `build_cmd` produces so a regression on the adapter shape is caught at `cargo test`
    //! time — no CLI installed, no network, no auth.

    use super::*;
    use std::ffi::OsStr;
    use std::path::PathBuf;

    fn args_of(cmd: &Command) -> Vec<String> {
        cmd.get_args().map(|a: &OsStr| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn claude_arm_composes_rich_invocation_in_the_expected_order() {
        let worktree = PathBuf::from("/tmp/fleet-worktree-fake");
        let cmd = build_cmd(CliAdapter::Claude, &worktree, "do the thing");

        assert_eq!(cmd.get_program(), OsStr::new("claude"));
        let args = args_of(&cmd);

        // Front of the arg list is the task under `-p`.
        assert_eq!(args[0], "-p");
        assert_eq!(args[1], "do the thing");

        // Flag order matters: --permission-mode, then --allowedTools, then
        // --append-system-prompt, then --add-dir. Assert exact positions.
        assert_eq!(args[2], "--permission-mode");
        assert_eq!(args[3], "acceptEdits");
        assert_eq!(args[4], "--allowedTools");
        assert_eq!(args[5], "Read Edit Write Bash Grep Glob");
        assert_eq!(args[6], "--append-system-prompt");
        // System prompt is the tuned body — assert non-empty rather than its full text.
        assert!(!args[7].is_empty(), "system prompt payload was empty");
        assert!(args[7].len() > 200, "system prompt looks too short to be the tuned body");
        assert_eq!(args[8], "--add-dir");
        assert_eq!(args[9], worktree.to_string_lossy());

        assert_eq!(args.len(), 10, "unexpected extra args: {args:?}");
    }

    #[test]
    fn claude_system_prompt_encodes_the_named_idioms() {
        // If either keyword disappears, the prompt has drifted from the tuned iteration-2
        // shape and the caller should re-benchmark before shipping.
        let prompt = fleet_worker::CLAUDE_SYSTEM_PROMPT;
        assert!(!prompt.is_empty(), "CLAUDE_SYSTEM_PROMPT is empty");
        assert!(
            prompt.contains("useState") || prompt.contains("Set<"),
            "CLAUDE_SYSTEM_PROMPT is missing the named-idiom keywords \
             (`useState` or `Set<`); re-check assets/claude-system-prompt.md"
        );
    }

    #[test]
    fn codex_arm_ends_with_exec_task_and_no_extra_flags() {
        let worktree = PathBuf::from("/tmp/fleet-worktree-fake");
        let cmd = build_cmd(CliAdapter::Codex, &worktree, "do the thing");

        assert_eq!(cmd.get_program(), OsStr::new("codex"));
        assert_eq!(args_of(&cmd), vec!["exec".to_string(), "do the thing".to_string()]);
    }

    #[test]
    fn freelane_arm_is_bare_task_with_no_extra_flags() {
        // Freelane never routes through run_cli in practice, but if a regression ever sends it
        // here, it must not silently inherit Claude's flags. Pinned as a bare `<task>`.
        let worktree = PathBuf::from("/tmp/fleet-worktree-fake");
        let cmd = build_cmd(CliAdapter::Freelane, &worktree, "do the thing");
        assert_eq!(args_of(&cmd), vec!["do the thing".to_string()]);
    }
}
