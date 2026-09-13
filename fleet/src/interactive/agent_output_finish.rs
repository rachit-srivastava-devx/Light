//! Final status projection kept separate from live streaming.

use super::super::theme::*;
use builder::CliAdapter;
use std::path::Path;

pub fn finish(
    state: &Path,
    adapter: CliAdapter,
    outcome: crate::dispatch::agent_cmd_run::AgentOutcome,
    color: bool,
) {
    match outcome {
        crate::dispatch::agent_cmd_run::AgentOutcome::Done {
            body,
            resolved_model,
            tokens,
        } => {
            super::super::outcome_receipt::record(
                state,
                adapter.agent_kind(),
                &body,
                resolved_model.as_deref(),
                tokens,
                true,
            );
            if !body
                .get("streamed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                print_response(&body);
            }
            println!(
                "\n  {} Done{}",
                paint(color, GREEN, "✓"),
                model_suffix(resolved_model, color)
            );
            match tokens {
                Some(count) => println!("  {} Reported tokens: {count}", paint(color, GRAY, "·")),
                None => println!(
                    "  {} Provider did not report token usage",
                    paint(color, GRAY, "·")
                ),
            }
        }
        crate::dispatch::agent_cmd_run::AgentOutcome::Refused(reason) => {
            let body = serde_json::json!({"reason": reason});
            super::super::outcome_receipt::record(
                state,
                adapter.agent_kind(),
                &body,
                None,
                None,
                false,
            );
            println!("  {} Refused: {reason}", paint(color, RED, "✗"));
        }
    }
    println!();
}

fn model_suffix(model: Option<String>, color: bool) -> String {
    model
        .filter(|value| !value.is_empty())
        .map(|value| format!("  (model: {})", paint(color, GRAY, &value)))
        .unwrap_or_default()
}

fn print_response(body: &serde_json::Value) {
    if let Some(text) = body.get("response").and_then(|value| value.as_str()) {
        for line in text.lines() {
            println!("  {line}");
        }
    }
}
