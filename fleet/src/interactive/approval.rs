//! Interactive approval: enumerate requested effects before an adapter receives the task.

use super::agent_loop::execute_task;
use super::effect_plan::{discover, label, Effect};
use super::line_reader::{LineReader, ReadOutcome};
use super::theme::*;
use std::path::Path;

pub async fn execute(
    task: &str,
    state_dir: &Path,
    model: &str,
    auto_mode: bool,
    color: bool,
    reader: &mut LineReader,
) {
    let effects = discover(task);
    if !requires_approval(&effects) {
        execute_task(task, state_dir, model, auto_mode, color, &effects).await;
        return;
    }
    println!(
        "\n  {}",
        paint(color, BOLD, "Permissions required before implementation:")
    );
    for effect in &effects {
        println!("    {} {}", paint(color, CYAN, "•"), label(*effect));
    }
    println!(
        "  {} Fleet will not publish without a separate approved task.",
        paint(color, GRAY, "Note:")
    );
    match reader.read_input("approve all listed effects? [y/N] ", color) {
        ReadOutcome::Submit(answer) if approved(&answer) => {
            execute_task(task, state_dir, model, auto_mode, color, &effects).await;
        }
        ReadOutcome::Exit => refuse(state_dir, task, &effects, "user cancelled approval", color),
        _ => refuse(state_dir, task, &effects, "user denied approval", color),
    }
}

pub fn approved(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// A model response alone is ordinary chat. Any local or external effect needs one complete
/// approval plan before the adapter receives the task.
pub fn requires_approval(effects: &[Effect]) -> bool {
    effects
        .iter()
        .any(|effect| !matches!(effect, Effect::Model))
}

fn refuse(state_dir: &Path, task: &str, effects: &[Effect], reason: &str, color: bool) {
    match record_refusal(state_dir, task, effects, reason) {
        Ok(()) => println!(
            "  {} Task refused; no agent was started.\n",
            paint(color, AMBER, "!")
        ),
        Err(error) => eprintln!("fleet: task refused but could not write receipt: {error}"),
    }
}

pub(super) fn record_refusal(
    state_dir: &Path,
    task: &str,
    effects: &[Effect],
    reason: &str,
) -> Result<(), String> {
    std::fs::create_dir_all(state_dir).map_err(|e| e.to_string())?;
    let effects: Vec<&str> = effects.iter().map(|effect| label(*effect)).collect();
    let body = serde_json::json!({ "task": task, "effects": effects, "reason": reason });
    crate::pipeline::ledger_events::append_as(
        state_dir,
        types::ReceiptEvent::Refusal,
        body,
        "fleet-cli-interactive",
    )
}
