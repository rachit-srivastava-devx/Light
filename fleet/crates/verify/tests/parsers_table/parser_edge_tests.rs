use super::*;

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
    let out = "corpus-gate: not-applicable -- target repo /some/user/repo is not a fleet checkout";
    assert_eq!(p(out, ""), DenominatorResult::NotApplicable);
}

#[test]
fn detectors_parses_plain_n_over_n() {
    let p = parser_for("detectors");
    assert_eq!(
        p("12 detectors match the manifest (denominator: 12)", ""),
        DenominatorResult::Counted(12, 12)
    );
}

#[test]
fn detectors_honours_not_applicable_marker_on_foreign_repos() {
    let p = parser_for("detectors");
    let out =
        "detectors-gate: not-applicable -- target repo /some/user/repo is not a fleet checkout";
    assert_eq!(p(out, ""), DenominatorResult::NotApplicable);
}

#[test]
fn unparseable_stdout_is_unparseable_for_every_gate() {
    for gate in GATES {
        assert_eq!(
            (gate.parse_denominator)("nothing recognizable here", ""),
            DenominatorResult::Unparseable
        );
    }
}
