//! The collapsed-message half of the D27 fix: "not opted in" and "not installed" used to both
//! print as `mutants unavailable`, misleading a user who genuinely has `cargo-mutants` on `$PATH`
//! into thinking they need to install something. Split out of `mutants_opt_in_real_binary.rs` to
//! keep both files under the 80-line cap. Drives the REAL compiled binary (see that file's doc
//! comment for why a fake probe wouldn't prove this).

mod support;
use std::time::Duration;
use support::bounded::run_bounded;

/// A machine that genuinely HAS `cargo-mutants` on `$PATH`, but with `FLEET_MUTANTS` unset: the
/// skip must name the opt-in cause, not a generic "unavailable".
#[test]
fn mutants_gate_skip_names_the_opt_in_cause_when_the_tool_is_present() {
    let repo = tempfile::tempdir().unwrap();
    support::scratch_repo(repo.path());
    let repo_arg = repo.path().to_string_lossy().into_owned();

    let result = run_bounded(
        &["gate", "--id", "mutants", "--repo", &repo_arg],
        &[("FLEET_MUTANTS", "0")],
        Duration::from_secs(10),
    );
    let (_code, out, err) = result.expect("an opt-out skip must terminate fast, not hang");
    assert!(
        err.contains("set FLEET_MUTANTS=1 to run it"),
        "opt-out skip must name the real cause, not a generic 'unavailable': stdout={out} stderr={err}"
    );
}

/// With `FLEET_MUTANTS=1` set but `cargo-mutants` absent from `$PATH`, the skip message must say
/// "not found on PATH" -- never the same text as the opt-out case above, and never "unavailable".
#[test]
fn mutants_gate_skip_names_the_path_cause_when_the_tool_is_absent() {
    let repo = tempfile::tempdir().unwrap();
    support::scratch_repo(repo.path());
    let repo_arg = repo.path().to_string_lossy().into_owned();

    // Restrict PATH AND route HOME+CARGO_HOME to an empty tempdir so that
    // tool_path::fallback_dirs() cannot find cargo-mutants in ~/.cargo/bin either
    // (the fallback exists for rustup installs missing from PATH, which would defeat this test).
    let fake_home = tempfile::tempdir().unwrap();
    let fake_home_arg = fake_home.path().to_string_lossy().into_owned();

    let result = run_bounded(
        &["gate", "--id", "mutants", "--repo", &repo_arg],
        &[
            ("FLEET_MUTANTS", "1"),
            ("PATH", "/usr/bin:/bin:/usr/sbin:/sbin"),
            ("HOME", &fake_home_arg),
            ("CARGO_HOME", &fake_home_arg),
        ],
        Duration::from_secs(10),
    );
    let (_code, out, err) = result.expect("a not-on-PATH skip must terminate fast, not hang");
    assert!(
        err.contains("cargo-mutants not found on PATH"),
        "not-installed skip must name the real cause: stdout={out} stderr={err}"
    );
}
