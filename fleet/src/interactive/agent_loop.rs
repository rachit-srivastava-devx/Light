//! Autonomous agent loop for free-text coding task prompts.
//!
//! Free-text execution is not wired to the real pipeline yet -- there is no `--repo` in an
//! interactive prompt, so this refuses rather than print a scripted "gates pass / attestation
//! recorded" transcript for work that never ran (AGENTS.md hard rule 6 / L8: never fabricate a
//! passing verdict for an unrun check).

use super::theme::*;
use std::path::Path;

pub async fn execute_task(task: &str, _state_dir: &Path, _model: &str, _auto_mode: bool, color: bool) {
    let warn = paint(color, AMBER, "⚠");
    println!();
    println!("  {warn} {}", paint(color, BOLD, "Not yet implemented: free-text task execution"));
    println!("    \"{task}\" was not run -- no worktree was spawned, no gate ran, nothing was attested.");
    println!("    Use `fleet run --repo <path> --task \"{task}\"` for the real, verified pipeline.");
    println!();
}
