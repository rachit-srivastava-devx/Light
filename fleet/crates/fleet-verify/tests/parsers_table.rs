//! One fixture per real gate script's stdout shape (copied verbatim from BLUEPRINT.md §5's
//! citations), asserting the exact `Counted(n, total)` each committed gate's parser produces.
//! Looked up by id in the committed registry rather than calling parser fns directly, since the
//! parsers are reviewed data (§5), not part of this crate's public API surface.

use fleet_verify::{DenominatorResult, GATES};

fn parser_for(id: &str) -> fn(&str, &str) -> DenominatorResult {
    GATES.iter().find(|g| g.id == id).unwrap_or_else(|| panic!("no gate {id}")).parse_denominator
}

#[test]
fn mutants_parses_caught_total_floor_line() {
    let p = parser_for("mutants");
    assert_eq!(p("mutants: caught=40 total=42 floor=0.90", ""), DenominatorResult::Counted(40, 42));
}

#[test]
fn semgrep_normalizes_scanned_findings_to_clean_over_scanned() {
    let p = parser_for("semgrep");
    assert_eq!(p("120 files scanned, 3 findings", ""), DenominatorResult::Counted(117, 120));
}

#[test]
fn trivy_normalizes_targets_findings_to_clean_over_targets() {
    let p = parser_for("trivy");
    assert_eq!(p("2 secret findings across 50 reported targets", ""), DenominatorResult::Counted(48, 50));
}

#[test]
fn recur_normalizes_checked_flagged_to_clean_over_checked() {
    let p = parser_for("recur");
    assert_eq!(p("recur-gate: checked=200 flagged=1 (signatures=1: E1)", ""), DenominatorResult::Counted(199, 200));
}

#[test]
fn detectors_parses_plain_n_over_n() {
    let p = parser_for("detectors");
    assert_eq!(p("12 detectors match the manifest (denominator: 12)", ""), DenominatorResult::Counted(12, 12));
}

#[test]
fn detectors_honours_not_applicable_marker_on_foreign_repos() {
    let p = parser_for("detectors");
    // The detectors gate hashes fleet's own detector inventory; on a user
    // repo the script prints this marker and exits 0. Parser must return
    // NotApplicable so the gate reads as SKIP, not FAIL -- otherwise every
    // `fleet run --repo <any user repo>` ends with `FAIL gate detectors --
    // NonZeroExit(6)`, a red gate that never had a chance to succeed.
    let out = "detectors-gate: not-applicable -- target repo /some/user/repo is not a fleet checkout";
    assert_eq!(p(out, ""), DenominatorResult::NotApplicable);
}

#[test]
fn policy_parses_passed_failed_denominator() {
    let p = parser_for("policy");
    assert_eq!(p("-- 9 passed, 1 failed (denominator: 10 policies) --", ""), DenominatorResult::Counted(9, 10));
}

#[test]
fn corpus_collapses_seven_field_line_to_clean_over_total() {
    let p = parser_for("corpus");
    let line = "DENOMINATOR checked=100 total=100 excluded=0 caught=95 timeout_contention=0 \
                timeout_confirmed=0 timeout_persistent=0";
    assert_eq!(p(line, ""), DenominatorResult::Counted(5, 100));
}

#[test]
fn unit_tests_parses_libtest_result_line() {
    let p = parser_for("unit tests");
    assert_eq!(p("test result: ok. 42 passed; 0 failed; 0 ignored", ""), DenominatorResult::Counted(42, 42));
}

#[test]
fn unit_tests_total_is_passed_plus_failed_not_passed_minus_failed() {
    let p = parser_for("unit tests");
    assert_eq!(p("test result: FAILED. 38 passed; 4 failed; 0 ignored", ""), DenominatorResult::Counted(38, 42));
}

#[test]
fn unparseable_stdout_is_unparseable_for_every_gate() {
    for gate in GATES {
        assert_eq!((gate.parse_denominator)("nothing recognizable here", ""), DenominatorResult::Unparseable);
    }
}
