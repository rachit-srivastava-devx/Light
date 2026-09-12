//! Claude Code-inspired header banner with pixel-art vessel mascot and status.

use super::theme::*;
use std::env;

pub fn render_banner(color: bool, model: &str, lanes: usize) {
    let cwd = env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".into());
    let version = env!("CARGO_PKG_VERSION");

    let mascot_colored: Vec<String> = MASCOT.iter().map(|line| paint(color, PURPLE, line)).collect();
    let title = format!("{} {}", paint(color, BOLD, "Fleet"), paint(color, GRAY, &format!("v{version}")));
    let meta = paint(color, GRAY, &format!("{model} · Local Orchestration"));
    let path = paint(color, DIM, &cwd);

    println!();
    println!("{}   {}", mascot_colored[0], title);
    println!("{}   {}", mascot_colored[1], meta);
    println!("{}   {}", mascot_colored[2], path);
    println!();
    println!("  {}", paint(color, WHITE, "Autonomous agent swarms in parallel worktrees. Switch anytime with /model."));
    println!();
    let check = paint(color, GREEN, "✓");
    let status_text = paint(color, GREEN, &format!("System capacity healthy · Ready ({lanes} lanes)"));
    println!("                                              {check} {status_text}");
    println!("{}", paint(color, GRAY, "──────────────────────────────────────────────────────────────────────────────"));
}
