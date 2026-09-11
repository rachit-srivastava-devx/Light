//! Bottom status footer reflecting active model, execution mode, and capacity.

use super::theme::*;

pub fn render_status_bar(color: bool, model: &str, lanes: usize, auto_mode: bool) {
    let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
    let sep = paint(color, GRAY, " | ");

    let line1 = format!(
        "  {}{sep}{}{sep}{}{sep}{}",
        paint(color, CYAN, model),
        paint(color, GREEN, "ready"),
        paint(color, WHITE, &user),
        paint(color, GRAY, &format!("Lanes:{lanes} Cap:{lanes}"))
    );

    let mode_indicator = if auto_mode {
        format!("{} {}", paint(color, BRIGHT_AMBER, "⏩"), paint(color, BRIGHT_AMBER, "auto mode on"))
    } else {
        format!("{} {}", paint(color, CYAN, "⏸ "), paint(color, CYAN, "review mode on"))
    };

    let line2 = format!(
        "  {} {} · {} · {}",
        mode_indicator,
        paint(color, GRAY, "(shift+tab to cycle)"),
        paint(color, GRAY, "/help for commands"),
        paint(color, GRAY, "Ctrl+C to exit")
    );

    println!();
    println!("{line1}");
    println!("{line2}");
}
