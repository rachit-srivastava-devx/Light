use super::*;

#[test]
fn unit_tests_parses_libtest_result_line() {
    let p = parser_for("unit tests");
    assert_eq!(
        p("test result: ok. 42 passed; 0 failed; 0 ignored", ""),
        DenominatorResult::Counted(42, 42)
    );
}

#[test]
fn unit_tests_total_is_passed_plus_failed_not_passed_minus_failed() {
    let p = parser_for("unit tests");
    assert_eq!(
        p("test result: FAILED. 38 passed; 4 failed; 0 ignored", ""),
        DenominatorResult::Counted(38, 42)
    );
}

#[test]
fn unit_tests_parses_jest_summary_line() {
    let p = parser_for("unit tests");
    let jest = "Test Suites: 1 passed, 1 total\nTests:       671 passed, 671 total\nSnapshots:   0 total\n";
    assert_eq!(p(jest, ""), DenominatorResult::Counted(671, 671));
    assert_eq!(p("", jest), DenominatorResult::Counted(671, 671));
    assert_eq!(
        p("Tests:       1 failed, 670 passed, 671 total\n", ""),
        DenominatorResult::Counted(670, 671)
    );
    assert_eq!(
        p(
            "Tests:       1 failed, 2 skipped, 670 passed, 673 total\n",
            ""
        ),
        DenominatorResult::Counted(670, 673)
    );
}

#[test]
fn unit_tests_parses_vitest_summary_line() {
    let p = parser_for("unit tests");
    assert_eq!(
        p(
            " Test Files  1 passed (1)\n      Tests  10 passed (10)\n",
            ""
        ),
        DenominatorResult::Counted(10, 10)
    );
    assert_eq!(
        p(
            " Test Files  2 failed | 5 passed (7)\n      Tests  3 failed | 10 passed (13)\n",
            ""
        ),
        DenominatorResult::Counted(10, 13)
    );
}
