//! Visual theme, ANSI escapes, and mascot artwork for the interactive DX.

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const CYAN: &str = "\x1b[36m";
#[allow(dead_code)]
pub const BRIGHT_CYAN: &str = "\x1b[96m";
pub const AMBER: &str = "\x1b[33m";
pub const BRIGHT_AMBER: &str = "\x1b[93m";
pub const GREEN: &str = "\x1b[32m";
pub const RED: &str = "\x1b[31m";
pub const GRAY: &str = "\x1b[90m";
pub const WHITE: &str = "\x1b[97m";

pub const MASCOT: &[&str] = &[
    "    ┏━━━━━━━━━━━┓    ",
    "━━━━┛           ┗━━━━",
    "    ┗━━━━━━━━━━━┛    ",
];

pub fn paint(color: bool, code: &str, text: &str) -> String {
    if color {
        format!("{code}{text}{RESET}")
    } else {
        text.to_string()
    }
}
