//! One fixture per real gate script's stdout shape (copied verbatim from BLUEPRINT.md §5's
//! citations), asserting the exact `Counted(n, total)` each committed gate's parser produces.
//! Looked up by id in the committed registry rather than calling parser fns directly, since the
//! parsers are reviewed data (§5), not part of this crate's public API surface.

use verify::{DenominatorResult, GATES};

fn parser_for(id: &str) -> fn(&str, &str) -> DenominatorResult {
    GATES.iter().find(|g| g.id == id).unwrap_or_else(|| panic!("no gate {id}")).parse_denominator
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
fn unit_tests_parses_jest_summary_line() {
    let p = parser_for("unit tests");
    let jest = "Test Suites: 1 passed, 1 total\nTests:       671 passed, 671 total\nSnapshots:   0 total\n";
    assert_eq!(p(jest, ""), DenominatorResult::Counted(671, 671));
    // Jest actually writes its summary to STDERR in normal runs -- proven against
    // posx-frido-backend `npm run --silent test:unit` (stdout empty, stderr carries the summary).
    assert_eq!(p("", jest), DenominatorResult::Counted(671, 671));

    let with_failed = "Tests:       1 failed, 670 passed, 671 total\n";
    assert_eq!(p(with_failed, ""), DenominatorResult::Counted(670, 671));

    let with_skipped = "Tests:       1 failed, 2 skipped, 670 passed, 673 total\n";
    assert_eq!(p(with_skipped, ""), DenominatorResult::Counted(670, 673));
}

#[test]
fn unit_tests_parses_vitest_summary_line() {
    let p = parser_for("unit tests");
    let vitest = " Test Files  1 passed (1)\n      Tests  10 passed (10)\n";
    assert_eq!(p(vitest, ""), DenominatorResult::Counted(10, 10));

    let vitest_mixed = " Test Files  2 failed | 5 passed (7)\n      Tests  3 failed | 10 passed (13)\n";
    assert_eq!(p(vitest_mixed, ""), DenominatorResult::Counted(10, 13));
}

#[test]
fn corpus_collapses_seven_field_line_to_clean_over_total() {
    let p = parser_for("corpus");
    let line = "DENOMINATOR checked=100 total=100 excluded=0 caught=95 timeout_contention=0 \
                timeout_confirmed=0 timeout_persistent=0";
    assert_eq!(p(line, ""), DenominatorResult::Counted(5, 100));
}

#[test]
fn corpus_honours_not_applicable_marker_on_foreign_repos() {
    let p = parser_for("corpus");
    // The whole corpus gate asserts fleet-own-source invariants; on a user
    // repo the script prints this marker and exits 0. Parser must return
    // NotApplicable so the gate reads as SKIP, not FAIL -- otherwise every
    // `fleet run --repo <any user repo>` ends with a red gate that never
    // had a chance to succeed.
    let out = "corpus-gate: not-applicable -- target repo /some/user/repo is not a fleet checkout";
    assert_eq!(p(out, ""), DenominatorResult::NotApplicable);
}

#[test]
fn unparseable_stdout_is_unparseable_for_every_gate() {
    for gate in GATES {
        assert_eq!((gate.parse_denominator)("nothing recognizable here", ""), DenominatorResult::Unparseable);
    }
}
