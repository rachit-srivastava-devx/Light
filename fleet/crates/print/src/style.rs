//! Terminal-honesty policy for the human render path: colour only when stderr is a real TTY,
//! `NO_COLOR` is unset, `TERM` isn't `dumb`, and `--no-color` wasn't passed. `Style::new` is the
//! pure constructor renderer tests use directly (no env/TTY probing in test code); `Style::detect`
//! is the one impure entry point real call sites use, reading the world exactly once per call.

use std::io::IsTerminal;
use std::sync::OnceLock;

static NO_COLOR_FLAG: OnceLock<bool> = OnceLock::new();

/// Set once, at startup, from the parsed `--no-color` CLI flag. Reading before it's set treats
/// the flag as absent (`false`) -- the honest default for any caller that runs before `main`
/// wires this (e.g. a unit test constructing `Style` directly should prefer `Style::new`).
pub fn set_no_color_flag(flag: bool) {
    let _ = NO_COLOR_FLAG.set(flag);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    pub color: bool,
}

impl Style {
    pub const fn new(color: bool) -> Self {
        Style { color }
    }

    pub fn detect() -> Self {
        let flag = NO_COLOR_FLAG.get().copied().unwrap_or(false);
        let env_no_color = std::env::var_os("NO_COLOR").is_some();
        let term_dumb = std::env::var("TERM").map(|t| t == "dumb").unwrap_or(false);
        let tty = std::io::stderr().is_terminal();
        Style::new(tty && !flag && !env_no_color && !term_dumb)
    }

    pub fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("{code}{text}{RESET}")
        } else {
            text.to_string()
        }
    }
}

#[cfg(test)]
#[path = "style_tests.rs"]
mod tests;

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const CYAN: &str = "\x1b[36m";
pub const MAGENTA: &str = "\x1b[35m";
