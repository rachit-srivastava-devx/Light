//! One small `parse_denominator` fn per gate family. "Lower is better" gates (semgrep/trivy)
//! are normalized to "higher numerator is better" (clean-of-total) so every `GateSpec` in the
//! registry shares one reading direction.

use crate::denominator::DenominatorResult as D;
use crate::digits::{after, before};

/// `mutants-gate.sh:106-111` -- `"mutants: caught=%d total=%d floor=%s"`.
pub fn mutants(stdout: &str, _stderr: &str) -> D {
    match after(stdout, "caught=").zip(after(stdout, "total=")) {
        Some((c, t)) => D::Counted(c, t),
        None => D::Unparseable,
    }
}

/// `semgrep-gate.sh:100-108` -- `"<N> files scanned, <M> findings"`, normalized to clean/scanned.
pub fn semgrep(stdout: &str, _stderr: &str) -> D {
    match before(stdout, " files scanned").zip(before(stdout, " findings")) {
        Some((scanned, findings)) => D::Counted(scanned.saturating_sub(findings), scanned),
        None => D::Unparseable,
    }
}

/// `trivy-gate.sh:61-67` -- `"<total> secret findings across <N> reported targets"`.
pub fn trivy(stdout: &str, _stderr: &str) -> D {
    match before(stdout, " secret findings").zip(before(stdout, " reported targets")) {
        Some((total, targets)) => D::Counted(targets.saturating_sub(total), targets),
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
pub fn corpus(stdout: &str, _stderr: &str) -> D {
    let fields = after(stdout, "checked=")
        .zip(after(stdout, "total="))
        .zip(after(stdout, "caught="));
    match fields {
        Some(((checked, total), caught)) => D::Counted(checked.saturating_sub(caught), total),
        None => D::Unparseable,
    }
}

/// libtest's own fixed `"test result: ok. N passed; M failed"` line -- greenfield, no fleet
/// script wraps this today.
pub fn unit_tests(stdout: &str, _stderr: &str) -> D {
    match before(stdout, " passed;").zip(before(stdout, " failed;")) {
        Some((p, f)) => D::Counted(p, p + f),
        None => D::Unparseable,
    }
}
