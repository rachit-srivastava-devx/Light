//! Regression pin for the DX-AUDIT finding that all 28 top-level `--help` descriptions were
//! empty and no test caught it. Drives the real binary's `--help` output (never a hand-copied
//! string) so a future subcommand added without an `about` in `cli::help_text` fails here. The
//! stale-`NOT IMPLEMENTED`-label check lives in `no_stale_not_implemented_labels.rs` (≤80-line
//! split).

mod support;
use support::cmd;

#[test]
fn every_top_level_subcommand_has_a_non_empty_description() {
    let out = cmd().arg("--help").output().expect("binary runs");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let commands_block = stdout.split("Commands:\n").nth(1).expect("a Commands: section");

    let mut missing = Vec::new();
    for line in commands_block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !line.starts_with("  ") {
            break; // end of the Commands: block (blank line before Options:)
        }
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or("");
        let desc = parts.next().unwrap_or("").trim();
        if name != "help" && desc.is_empty() {
            missing.push(name.to_string());
        }
    }
    assert!(missing.is_empty(), "subcommand(s) with an empty --help description: {missing:?}");
}
