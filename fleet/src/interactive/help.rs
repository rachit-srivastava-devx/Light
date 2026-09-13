//! Interactive help text and command descriptions.

use super::theme::*;

pub fn print_help(color: bool) {
    let b = |s: &str| paint(color, BOLD, s);
    let c = |s: &str| paint(color, CYAN, s);
    let g = |s: &str| paint(color, GRAY, s);

    println!();
    println!("  {}", b("Fleet Interactive Commands:"));
    println!("    {}        {}", c("/help"), g("Show this help menu"));
    println!(
        "    {}       {}",
        c("/doctor"),
        g("Inspect host tools, memory, and capacity")
    );
    println!(
        "    {}      {}",
        c("/status"),
        g("Display active concurrency cap and lane state")
    );
    println!(
        "    {}   {}",
        c("/model [name]"),
        g("View or switch the active LLM model")
    );
    println!(
        "    {}       {}",
        c("/gates"),
        g("List verification gates and acceptance checks")
    );
    println!(
        "    {}        {}",
        c("/mode"),
        g("Toggle between auto and review execution modes")
    );
    println!(
        "    {}       {}",
        c("/clear"),
        g("Clear terminal screen and repaint banner")
    );
    println!(
        "    {}  {}",
        c("/exit, /quit"),
        g("Exit the interactive session")
    );
    println!();
    println!("  {}", b("Keyboard Shortcuts:"));
    println!(
        "    {}     {}",
        c("Shift+Tab"),
        g("Instantly cycle execution mode (auto / review)")
    );
    println!(
        "    {}      {}",
        c("Up / Down"),
        g("Navigate command and prompt history")
    );
    println!(
        "    {}        {}",
        c("Ctrl+C"),
        g("Cancel current input or exit")
    );
    println!("    {}        {}", c("Ctrl+L"), g("Clear the screen"));
    println!();
    println!("  {}", b("Natural Language Prompts:"));
    println!(
        "    {}  {}",
        g("Type any coding task to initiate autonomous SOW, worktrees, and gates:"),
        c("> add unit tests for user auth")
    );
    println!();
}
