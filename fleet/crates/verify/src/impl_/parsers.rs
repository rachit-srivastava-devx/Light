//! One small `parse_denominator` fn per gate family. "Lower is better" gates (semgrep/trivy)
//! are normalized to "higher numerator is better" (clean-of-total) so every `GateSpec` in the
//! registry shares one reading direction.

use super::denominator::DenominatorResult as D;
use super::digits::{after, before};

#[path = "parsers_scanners.rs"]
mod parsers_scanners;
#[path = "parsers_suite.rs"]
mod parsers_suite;
pub use parsers_scanners::{semgrep, trivy};
pub use parsers_suite::unit_tests;

/// `mutants-gate.sh:106-111` -- `"mutants: caught=%d total=%d floor=%s"`.
pub fn mutants(stdout: &str, _stderr: &str) -> D {
    match after(stdout, "caught=").zip(after(stdout, "total=")) {
        Some((c, t)) => D::Counted(c, t),
        None => D::Unparseable,
    }
}

/// `recur-gate.sh:146` -- `"recur-gate: checked=%d flagged=%d"`, plus the explicit
/// not-applicable marker for a tree with no diff to examine (see `DenominatorResult`).
pub fn recur(stdout: &str, _stderr: &str) -> D {
    if stdout.contains("recur-gate: not-applicable") {
        return D::NotApplicable;
    }
    match after(stdout, "checked=").zip(after(stdout, "flagged=")) {
        Some((c, f)) => D::Counted(c.saturating_sub(f), c),
        None => D::Unparseable,
    }
}

/// `detector-integrity.sh:29` -- `"$N detectors match the manifest (denominator: $N)"`.
/// Also honours the explicit not-applicable marker the script emits on a
/// foreign (non-fleet) checkout: the gate hashes fleet's OWN detector
/// inventory, so a `fleet run` against a user repo has nothing to verify.
/// Same `NotApplicable` treatment as `recur` and `corpus`.
pub fn detectors(stdout: &str, _stderr: &str) -> D {
    if stdout.contains("detectors-gate: not-applicable") {
        return D::NotApplicable;
    }
    match after(stdout, "(denominator: ") {
        Some(n) => D::Counted(n, n),
        None => D::Unparseable,
    }
}

/// `policy/run.sh` -- `"-- $P passed, $F failed (denominator: $((P+F)) policies) --"`.
pub fn policy(stdout: &str, _stderr: &str) -> D {
    match before(stdout, " passed,").zip(before(stdout, " failed")) {
        Some((p, f)) => D::Counted(p, p + f),
        None => D::Unparseable,
    }
}

/// `gates/corpus/run.sh` -- `"DENOMINATOR checked=%d total=%d ... caught=%d ..."`.
/// Also honours the explicit not-applicable marker the script emits on a
/// foreign (non-fleet) checkout: every detector under `gates/corpus/` asserts
/// invariants about fleet's own source, so a `fleet run` against a user repo
/// has nothing to examine there. Same `NotApplicable` treatment as `recur`.
pub fn corpus(stdout: &str, _stderr: &str) -> D {
    if stdout.contains("corpus-gate: not-applicable") {
        return D::NotApplicable;
    }
    let fields = after(stdout, "checked=")
        .zip(after(stdout, "total="))
        .zip(after(stdout, "caught="));
    match fields {
        Some(((checked, total), caught)) => D::Counted(checked.saturating_sub(caught), total),
        None => D::Unparseable,
    }
}

// `unit_tests` (a repo's own test-runner summary, three formats) lives in `parsers_suite.rs`.

#[cfg(test)]
#[path = "parsers_tests.rs"]
mod tests;
