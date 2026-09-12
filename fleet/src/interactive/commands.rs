//! Slash command dispatcher for the interactive session.

use super::help::print_help;
use super::theme::*;
use crate::runtime::ConcurrencyCap;

pub enum CommandOutcome {
    Continue,
    ToggleMode,
    SetModel(String),
    Clear,
    Exit,
}

pub fn execute_slash(cmd: &str, cap: ConcurrencyCap, model: &str, color: bool) -> CommandOutcome {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let root = parts.first().copied().unwrap_or("");
    match root {
        "/help" => {
            print_help(color);
            CommandOutcome::Continue
        }
        "/doctor" => {
            let _ = crate::dispatch::ops_cmd::doctor(false);
            CommandOutcome::Continue
        }
        "/status" => {
            let _ = crate::dispatch::ops_cmd::status(false, cap);
            CommandOutcome::Continue
        }
        "/gates" => {
            println!("  {}", paint(color, BOLD, "Configured Verification Gates:"));
            for gate in verify::GATES {
                let req = match gate.requirement {
                    verify::Requirement::Required => "required",
                    verify::Requirement::Advisory => "advisory",
                };
                println!("    {} {:<6} {}", paint(color, CYAN, "•"), gate.id, paint(color, GRAY, req));
            }
            CommandOutcome::Continue
        }
        "/model" => {
            if let Some(new_m) = parts.get(1) {
                println!("  {} Model switched to: {}", paint(color, GREEN, "✓"), paint(color, BOLD, new_m));
                CommandOutcome::SetModel((*new_m).to_string())
            } else {
                println!("  Active model: {} (pass `/model <name>` to switch)", paint(color, CYAN, model));
                CommandOutcome::Continue
            }
        }
        "/mode" => CommandOutcome::ToggleMode,
        "/clear" => CommandOutcome::Clear,
        "/exit" | "/quit" => CommandOutcome::Exit,
        unknown => {
            println!("  {} Unknown command: {unknown}. Type {} for available commands.", paint(color, RED, "error:"), paint(color, CYAN, "/help"));
            CommandOutcome::Continue
        }
    }
}
