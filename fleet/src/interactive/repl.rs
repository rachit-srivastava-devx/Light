//! Interactive REPL loop wiring banner, status bar, input reader, and execution.

use super::agent_loop::execute_task;
use super::banner::render_banner;
use super::commands::{execute_slash, CommandOutcome};
use super::line_reader::{read_input, ReadOutcome};
use super::status_bar::render_status_bar;
use super::theme::*;
use crate::runtime::ConcurrencyCap;
use std::path::Path;

pub async fn run_loop(state_dir: &Path, cap: ConcurrencyCap) {
    let color = crate::print::style::Style::detect().color;
    let lanes = cap.get();
    let mut model = std::env::var("FLEET_MODEL").unwrap_or_else(|_| "claude-3-7-sonnet".into());
    let mut auto_mode = true;
    let mut history: Vec<String> = Vec::new();

    render_banner(color, &model, lanes);
    render_status_bar(color, &model, lanes, auto_mode);

    loop {
        match read_input("› ", color, &history) {
            ReadOutcome::ToggleMode => {
                auto_mode = !auto_mode;
                render_status_bar(color, &model, lanes, auto_mode);
            }
            ReadOutcome::Clear => {
                print!("\x1b[2J\x1b[H");
                render_banner(color, &model, lanes);
                render_status_bar(color, &model, lanes, auto_mode);
            }
            ReadOutcome::Exit => break,
            ReadOutcome::Submit(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() { continue; }
                history.push(line.clone());
                if trimmed.starts_with('/') {
                    match execute_slash(trimmed, cap, &model, color) {
                        CommandOutcome::ToggleMode => {
                            auto_mode = !auto_mode;
                            render_status_bar(color, &model, lanes, auto_mode);
                        }
                        CommandOutcome::SetModel(m) => {
                            model = m;
                            render_status_bar(color, &model, lanes, auto_mode);
                        }
                        CommandOutcome::Clear => {
                            print!("\x1b[2J\x1b[H");
                            render_banner(color, &model, lanes);
                            render_status_bar(color, &model, lanes, auto_mode);
                        }
                        CommandOutcome::Exit => break,
                        CommandOutcome::Continue => {}
                    }
                } else {
                    execute_task(trimmed, state_dir, &model, auto_mode, color).await;
                    render_status_bar(color, &model, lanes, auto_mode);
                }
            }
        }
    }
    println!("  {}", paint(color, DIM, "Goodbye."));
}
