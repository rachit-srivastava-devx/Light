//! The marker `install.sh:foreign_bin` greps for MUST exist in the real binary's output.
//!
//! It did not. `install.sh` grepped `--help` for `'frozen, attested change'`, a string absent from
//! this entire repo, so every re-install classified fleet's OWN installed binary as foreign and
//! displaced it. Nothing watched the marker, so the check had quietly stopped measuring. This test
//! is the watch: it reads the pattern out of `install.sh` itself and asserts the shipped binary
//! actually prints it.

use std::process::Command;

/// Pulls the grep pattern out of `install.sh` rather than hardcoding it, so editing one side
/// without the other fails here instead of silently in the installer.
fn marker_from_install_sh(script: &str) -> String {
    let line = script
        .lines()
        .find(|l| l.contains("&& return 1") && l.contains("grep -q"))
        .expect("install.sh must still identify its own binary by an output marker");
    let after = line.split("grep -q").nth(1).expect("grep -q takes a pattern");
    let quoted = after.trim().trim_start_matches('\'');
    let pattern = quoted.split('\'').next().expect("pattern is single-quoted");
    pattern.trim_start_matches('^').to_string()
}

#[test]
fn the_installers_identity_marker_appears_in_the_real_binary_output() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../install.sh");
    let script = std::fs::read_to_string(root).expect("install.sh must exist next to the crate");
    let marker = marker_from_install_sh(&script);
    assert!(!marker.is_empty(), "extracted an empty marker from install.sh");

    // The real product binary, not a fake: this is the exact program `install.sh` interrogates.
    let out = Command::new(env!("CARGO_BIN_EXE_fleet"))
        .arg("version")
        .output()
        .expect("the fleet binary must run");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.lines().any(|l| l.starts_with(&marker)),
        "install.sh greps `version` for {marker:?} but the binary printed:\n{stdout}"
    );
}

#[test]
fn the_marker_extractor_itself_fails_on_a_missing_marker() {
    // A detector never seen to fail is not a detector.
    let bogus = "foreign_bin() {\n    return 0\n}\n";
    assert!(std::panic::catch_unwind(|| marker_from_install_sh(bogus)).is_err());
}
