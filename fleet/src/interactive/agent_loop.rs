//! Autonomous agent loop for free-text coding task prompts.
//!
//! Uses the current working directory as the worktree. Adapter selection priority:
//! claude (if on PATH) → codex (if on PATH) → freelane (embedded, always available).

use super::theme::*;
use builder::CliAdapter;
use std::path::Path;
use std::process::{Command, Stdio};

fn on_path(bin: &str) -> bool {
    Command::new("which").arg(bin).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

fn detect_adapter() -> CliAdapter {
    for adapter in [CliAdapter::Claude, CliAdapter::Codex] {
        if let Some(bin) = adapter.cli_binary_name() {
            if on_path(bin) {
                return adapter;
            }
        }
    }
    CliAdapter::Freelane
}

pub async fn execute_task(task: &str, _state_dir: &Path, model: &str, _auto_mode: bool, color: bool) {
    let worktree = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            let err = paint(color, RED, "✗");
            println!("\n  {err} Cannot determine working directory: {e}\n");
            return;
        }
    };

    let adapter = detect_adapter();
    let adapter_name = paint(color, CYAN, adapter.agent_kind());
    let task_display = paint(color, BOLD, task);
    println!();
    println!("  {} Running via {} in {}", paint(color, GREEN, "▶"), adapter_name, worktree.display());
    println!("  {} {task_display}", paint(color, GRAY, "›"));
    println!();

    let model_opt = if model.is_empty() { None } else { Some(model) };
    let outcome = tokio::task::spawn_blocking({
        let worktree = worktree.clone();
        let task = task.to_string();
        let model_str = model_opt.map(str::to_string);
        move || crate::dispatch::agent_cmd_run::run(adapter, &worktree, &task, model_str.as_deref())
    })
    .await
    .unwrap_or_else(|e| crate::dispatch::agent_cmd_run::AgentOutcome::Refused(format!("task panicked: {e}")));

    match outcome {
        crate::dispatch::agent_cmd_run::AgentOutcome::Done { body, resolved_model } => {
            let check = paint(color, GREEN, "✓");
            let used_model = resolved_model.unwrap_or_default();
            if !used_model.is_empty() {
                println!("  {check} Done  (model: {})", paint(color, GRAY, &used_model));
            } else {
                println!("  {check} Done");
            }
            if let Some(resp) = body.get("response").and_then(|v| v.as_str()) {
                if !resp.is_empty() {
                    println!();
                    for line in resp.lines() {
                        println!("  {line}");
                    }
                }
            }
            println!();
        }
        crate::dispatch::agent_cmd_run::AgentOutcome::Refused(reason) => {
            let x = paint(color, RED, "✗");
            println!("  {x} Refused: {reason}");
            println!();
        }
    }
}
