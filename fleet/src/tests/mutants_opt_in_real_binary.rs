//! D27: the `mutants` gate must stay opt-in, and a skipped opt-in must stay VISIBLE. A merge once
//! silently reverted the `FLEET_MUTANTS` guard and nothing caught it -- the gate still PASSED, it
//! just took ~24 minutes instead of ~90 seconds because `cargo-mutants` happened to be on `$PATH`
//! (see `dispatch::verify_ports::WhichProbe::available`). Drives the REAL compiled binary, not a
//! fake probe -- a fake would prove the trait, not the wiring a real machine actually exercises.

mod support;
use support::bounded::run_bounded;
use std::time::Duration;

/// Without the opt-in env var, `cargo-mutants` being on the test machine's `$PATH` must NOT be
/// enough to run it -- the gate must skip, and that skip must print (never a silent pass).
#[test]
fn mutants_gate_skips_visibly_without_the_opt_in_env_var() {
    let repo = tempfile::tempdir().unwrap();
    support::scratch_repo(repo.path());
    let repo_arg = repo.path().to_string_lossy().into_owned();

    // Force the negative case regardless of this process's own ambient environment.
    let result = run_bounded(
        &["gate", "--id", "mutants", "--repo", &repo_arg],
        &[("FLEET_MUTANTS", "0")],
        Duration::from_secs(10),
    );
    let (code, out, err) = result.expect("an opt-out skip must terminate fast, not hang");
    assert_eq!(code, 0, "an advisory skip must not fail the run: stdout={out} stderr={err}");
    assert!(
        err.contains("SKIP") && err.contains("mutants"),
        "opt-in skip must be VISIBLE (D27), got stdout={out} stderr={err}"
    );
}

/// With `FLEET_MUTANTS=1` set, the gate must actually attempt to run rather than skip -- proving
/// the guard is a real opt-in gate, not a permanently-disabled one.
#[test]
fn mutants_gate_actually_runs_when_opted_in() {
    let repo = tempfile::tempdir().unwrap();
    support::scratch_repo(repo.path());
    let repo_arg = repo.path().to_string_lossy().into_owned();

    let result = run_bounded(
        &["gate", "--id", "mutants", "--repo", &repo_arg],
        &[("FLEET_MUTANTS", "1"), ("FLEET_VERIFY_BUDGET_SECS", "2")],
        Duration::from_secs(15),
    );
    let (_code, out, err) = result.expect("an opted-in mutants run must still terminate within budget");
    assert!(
        !out.contains("SKIP") && !err.contains("SKIP"),
        "FLEET_MUTANTS=1 must make the gate attempt to run, not skip: stdout={out} stderr={err}"
    );
}
