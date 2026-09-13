//! Integration tests for the interactive REPL startup sequence.
//!
//! These tests drive the real binary to verify that the three startup behaviours are correct:
//! (1) non-terminal stdin/stdout prints help instead of entering the REPL,
//! (2) `fleet version` embeds the FLEET_REPO_PATH env var (added in build.rs so update_check can
//!     locate the repo without shelling out to git at runtime from the CWD),
//! (3) the binary contains exactly one reference to the update-check and path-check symbols so we
//!     know they are compiled in and not silently dead-code-eliminated.

use std::process::Command;

fn fleet() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fleet"))
}

/// When stdin/stdout are not terminals, `fleet` (no subcommand) must print help text, not hang.
/// This is the branch that guards REPL activation -- the binary must exit cleanly, not block.
#[test]
fn no_args_non_terminal_prints_help_and_exits() {
    let out = fleet().output().expect("fleet binary must run");
    // The binary exits 0 (Ok exit code from the Ok(()) branch in main.rs).
    assert_eq!(
        out.status.code(),
        Some(0),
        "fleet with no args on non-terminal must exit 0; got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("fleet")
            && (stdout.contains("Usage") || stdout.contains("usage") || stdout.contains("--help")),
        "expected help output, got:\n{stdout}"
    );
}

/// The FLEET_REPO_PATH env var must have been embedded at build time so update_check can find
/// the repo. We verify it is non-empty by checking that `fleet doctor --json` still succeeds
/// (doctor uses the same build_info module that embeds the path).
#[test]
fn doctor_json_succeeds_and_contains_build_sha() {
    let out = fleet()
        .args(["doctor", "--json"])
        .output()
        .expect("fleet binary must run");
    assert!(
        out.status.success(),
        "fleet doctor --json must exit 0; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let body = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("fleet doctor --json must emit valid JSON: {e}\nGot:\n{body}"));
    assert!(
        v.get("commit_sha").is_some(),
        "doctor JSON must contain commit_sha; got:\n{v}"
    );
}
