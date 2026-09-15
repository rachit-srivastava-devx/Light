//! Autonomous agent loop for free-text coding task prompts.
//!
//! Uses the current working directory as the worktree. Adapter selection priority:
//! claude (if on PATH) → codex (if on PATH) → freelane (embedded, always available).

use super::agent_adapter::detect_adapter;
use super::agent_output::{announce, finish, StreamPrinter};
use super::effect_plan::Effect;
use print::human_stream::emit;
use print::render_event::Event;
use print::style::Style;
use std::path::Path;

fn emit_lld_path() {
    let path = [
        "LLD §3 User CLI",
        "interactive answer path (pipeline::run_pipeline not traversed)",
        "adapter discovery and route (direct composition wiring)",
        "LLD §3 Worker adapter",
        "answer stream -> final response",
    ];
    let style = Style::detect();
    for (offset, node) in path.iter().enumerate() {
        emit(
            &Event::LldPathStep {
                stage: "interactive-answer".into(),
                index: offset + 1,
                total: path.len(),
                node: (*node).into(),
            },
            &style,
        );
    }
}

pub async fn execute_task(
    task: &str,
    state_dir: &Path,
    model: &str,
    _auto_mode: bool,
    color: bool,
    effects: &[Effect],
) {
    let worktree = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            super::agent_output::cwd_error(color, &e);
            return;
        }
    };

    let adapter = detect_adapter();
    emit_lld_path();
    announce(color, adapter, task, &worktree);

    let model_opt = if model.is_empty() { None } else { Some(model) };
    let policy = crate::dispatch::agent_cmd_run::AgentToolPolicy::approved(
        effects.contains(&Effect::LocalWrite),
        effects.contains(&Effect::LocalCommand),
    );
    let outcome = tokio::task::spawn_blocking({
        let worktree = worktree.clone();
        let task = task.to_string();
        let model_str = model_opt.map(str::to_string);
        move || {
            let mut stream = StreamPrinter::new();
            crate::dispatch::agent_cmd_run::run_with_progress(
                adapter,
                &worktree,
                &task,
                model_str.as_deref(),
                policy,
                |delta| stream.push(delta),
            )
        }
    })
    .await
    .unwrap_or_else(|e| {
        crate::dispatch::agent_cmd_run::AgentOutcome::Refused(format!("task panicked: {e}"))
    });

    finish(state_dir, adapter, outcome, color);
}
