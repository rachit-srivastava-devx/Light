//! Terminal projection and receipt recording for interactive agent outcomes.

use super::theme::*;
use builder::CliAdapter;
use std::io::Write;
use std::path::Path;

#[path = "agent_output_finish.rs"]
mod agent_output_finish;

pub use agent_output_finish::finish;

pub fn cwd_error(color: bool, error: &std::io::Error) {
    println!(
        "\n  {} Cannot determine working directory: {error}\n",
        paint(color, RED, "✗")
    );
}

pub fn announce(color: bool, adapter: CliAdapter, task: &str, worktree: &Path) {
    println!(
        "\n  {} Running via {} in {}",
        paint(color, GREEN, "▶"),
        paint(color, CYAN, adapter.agent_kind()),
        worktree.display()
    );
    println!(
        "  {} {}\n",
        paint(color, GRAY, "›"),
        paint(color, BOLD, task)
    );
}

pub struct StreamPrinter(bool);

impl StreamPrinter {
    pub fn new() -> Self {
        Self(false)
    }

    pub fn push(&mut self, delta: &str) {
        if !self.0 {
            print!("  ");
            self.0 = true;
        }
        print!("{}", indent(delta));
        let _ = std::io::stdout().flush();
    }
}

pub(super) fn indent(delta: &str) -> String {
    delta.replace('\n', "\n  ")
}

#[cfg(test)]
#[path = "agent_output_tests.rs"]
mod tests;
