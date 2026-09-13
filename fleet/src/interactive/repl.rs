//! Interactive REPL loop wiring banner, status bar, input reader, and execution.

use super::approval;
use super::banner::render_banner;
use super::commands::{execute_slash, CommandOutcome};
use super::line_reader::{LineReader, ReadOutcome};
use super::status_bar::render_status_bar;
use super::theme::*;
use crate::runtime::ConcurrencyCap;
use std::path::Path;

pub(super) const DEFAULT_MODEL: &str = "sonnet";

pub async fn run_loop(state_dir: &Path, cap: ConcurrencyCap) {
    let color = crate::print::style::Style::detect().color;
    let lanes = cap.get();
    let mut model = std::env::var("FLEET_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into());
    let mut auto_mode = true;
    let mut reader = LineReader::new();

    render_banner(color, &model, lanes);

    // Path check is synchronous and fast (filesystem only).
    if let Some(warn) = super::path_check::check(color) {
        println!("{warn}");
    }
    // Update check is a network call (git ls-remote, up to a 2s timeout inside
    // update_check::check itself). It used to be awaited here, ahead of the first prompt --
    // on a slow or unreachable remote that made every session pay up to 2s (measured ~0.7s on
    // a reachable one) before the user could type anything, for an advisory notice nobody is
    // blocked on. Run it in the background instead and surface it the first time the loop has
    // an idle moment (right before a status-bar redraw), never delaying input.
    let mut update_check = Some(tokio::spawn(super::update_check::check(color)));

    render_status_bar(color, &model, lanes, auto_mode);

    loop {
        match reader.read_input("› ", color) {
            ReadOutcome::ToggleMode => {
                auto_mode = !auto_mode;
                announce_update_if_ready(&mut update_check).await;
                render_status_bar(color, &model, lanes, auto_mode);
            }
            ReadOutcome::Clear => {
                print!("\x1b[2J\x1b[H");
                render_banner(color, &model, lanes);
                announce_update_if_ready(&mut update_check).await;
                render_status_bar(color, &model, lanes, auto_mode);
            }
            ReadOutcome::Exit => break,
            ReadOutcome::Submit(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
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
                    approval::execute(trimmed, state_dir, &model, auto_mode, color, &mut reader)
                        .await;
                    announce_update_if_ready(&mut update_check).await;
                    render_status_bar(color, &model, lanes, auto_mode);
                }
            }
        }
    }
    println!("  {}", paint(color, DIM, "Goodbye."));
}

/// Prints the background update check's notice the first time it has finished, then never
/// polls it again (the handle is taken). A silent `None` -- what this probe always returns
/// today, see `update_check::check`'s own doc comment -- prints nothing, matching prior behavior
/// exactly; only the blocking wait for that `None` is gone.
async fn announce_update_if_ready(update_check: &mut Option<tokio::task::JoinHandle<Option<String>>>) {
    let ready = matches!(update_check, Some(handle) if handle.is_finished());
    if !ready {
        return;
    }
    if let Ok(Some(notice)) = update_check.take().unwrap().await {
        println!("{notice}");
    }
}
