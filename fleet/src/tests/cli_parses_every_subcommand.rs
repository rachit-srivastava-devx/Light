//! Drives the real compiled `fleet` binary (BLUEPRINT §9): every documented subcommand string
//! parses via `--help` (clap prints usage and exits 0 without touching business logic), and an
//! unknown subcommand fails at parse time, not at dispatch.

mod support;
use support::cmd;

const SUBCOMMANDS: &[&str] = &[
    "meter", "route", "roles", "swarm", "sow", "plan", "skills", "role-check", "agents",
    "lifecycle", "run", "oracle", "adjudicate", "attest", "pr", "status", "rollback", "ledger",
    "contract", "gate", "freeze", "console", "graph", "impact", "mcp", "completions", "doctor",
    "version",
];

#[test]
fn every_documented_subcommand_parses_without_panicking() {
    for name in SUBCOMMANDS {
        let out = cmd().args([*name, "--help"]).output().expect("binary runs");
        assert!(out.status.success(), "`fleet {name} --help` failed:\n{}", String::from_utf8_lossy(&out.stderr));
    }
}

#[test]
fn unknown_subcommand_fails_at_parse_not_at_dispatch() {
    let out = cmd().arg("bogus-subcommand").output().expect("binary runs");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unrecognized") || stderr.contains("error"), "stderr: {stderr}");
}

#[test]
fn help_and_completions_are_generated_not_hand_written() {
    let out = cmd().arg("--help").output().expect("binary runs");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for name in ["meter", "route", "doctor", "version"] {
        assert!(stdout.contains(name), "help text missing `{name}`: {stdout}");
    }

    for shell in ["bash", "zsh", "fish"] {
        let out = cmd().args(["completions", shell]).output().expect("binary runs");
        assert!(out.status.success());
        assert!(!out.stdout.is_empty(), "{shell} completions were empty");
    }
}
