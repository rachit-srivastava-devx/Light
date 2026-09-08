//! Golden tests for `render_summary`. Pins "lead with `next:`" and the fix this file exists for:
//! `gates` (attempted) and `checks` (examined) are different units and must never share a
//! fraction -- `checked 0/8` used to read as both "nothing checked" and "8 checked" at once.

use super::{render_summary, Checks, Summary};
use crate::print::style::Style;

const PLAIN: Style = Style::new(false);

fn base(checks: Checks) -> Summary {
    Summary { ran: 8, passed: 6, failed: 2, skipped: 0, checks, refusals: vec![], next_action: None }
}

#[test]
fn failure_leads_with_next_action_before_the_recap() {
    let mut s = base(Checks::Performed { checked: 6, total: 8 });
    s.next_action = Some("fix and re-run: fleet gate --id \"semgrep\"".into());
    let out = render_summary(&s, &PLAIN);
    let first_line = out.lines().next().unwrap();
    assert_eq!(first_line, "next: fix and re-run: fleet gate --id \"semgrep\"");
    assert!(out.contains("gates    8 attempted -- 6 passed, 2 failed, 0 skipped"), "{out}");
}

#[test]
fn success_has_no_next_line_and_publishes_gates_and_checks_separately() {
    let mut s = base(Checks::Performed { checked: 5, total: 5 });
    (s.ran, s.passed, s.failed) = (5, 5, 0);
    let out = render_summary(&s, &PLAIN);
    assert!(!out.starts_with("next:"), "a clean run must not fabricate a next action: {out}");
    assert!(out.contains("gates    5 attempted -- 5 passed, 0 failed, 0 skipped"), "{out}");
    assert!(out.contains("checks   5/5 performed"), "{out}");
}

#[test]
fn gates_and_checks_are_never_printed_as_one_fraction() {
    // 8 gates attempted, every one failed before examining any input -- the old code printed the
    // nonsensical `checked 0/8`, conflating "checks performed" with "gates attempted".
    let s = base(Checks::Performed { checked: 0, total: 0 });
    let out = render_summary(&s, &PLAIN);
    assert!(!out.contains("checked 0/8"), "must not conflate gates-attempted with checks: {out}");
    assert!(out.contains("gates    8 attempted -- 6 passed, 2 failed, 0 skipped"), "{out}");
    assert!(out.contains("checks   0 performed"), "{out}");
}

#[test]
fn checked_zero_is_flagged_loudly_as_a_failure_disguised_as_a_result() {
    let s = base(Checks::Performed { checked: 0, total: 0 });
    let out = render_summary(&s, &PLAIN);
    let checks_line = out.lines().find(|l| l.starts_with("checks")).expect("a checks line");
    assert!(
        checks_line.contains("no gate examined any input"),
        "checked==0 must say so loudly, not just print a bare 0: {checks_line}"
    );
}

#[test]
fn real_check_counts_are_shown_when_gates_publish_them() {
    let mut s = base(Checks::Performed { checked: 422, total: 422 });
    (s.ran, s.passed, s.failed) = (9, 9, 0);
    assert!(render_summary(&s, &PLAIN).contains("checks   422/422 performed"));
}

#[test]
fn not_applicable_omits_the_checks_line_entirely() {
    // `fleet run`'s single-stage summary has no gate-published denominator -- it must not
    // fabricate a `checks 0 performed` line, which would falsely read as "nothing examined".
    let out = render_summary(&base(Checks::NotApplicable), &PLAIN);
    assert!(!out.lines().any(|l| l.starts_with("checks")), "{out}");
}

#[test]
fn refusals_are_each_a_separate_attributed_line() {
    let mut s = base(Checks::NotApplicable);
    (s.ran, s.passed, s.failed) = (1, 0, 1);
    s.refusals = vec![("cargo".into(), "verify budget already spent".into())];
    let out = render_summary(&s, &PLAIN);
    assert!(out.contains("refused cargo: verify budget already spent"), "{out}");
}
