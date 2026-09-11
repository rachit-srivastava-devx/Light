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
pub fn detectors(stdout: &str, _stderr: &str) -> D {
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

/// A repo's unit-test suite. Handles three summary shapes so `.fleet/gates.toml` can point at
/// any of them without a per-repo parser:
///
/// * **libtest** (Rust) -- `"test result: ok. N passed; M failed; ..."` (semicolons).
/// * **Jest / npm test** -- `"Tests:       N passed, T total"` (commas; may include `failed`
///   and/or `skipped` counts before `passed`).
/// * **Vitest** -- `"      Tests  N passed (T)"` (parens; may include `failed | passed`).
///
/// `libtest` is tried first because its `passed;` / `failed;` markers are unambiguous. `Jest`
/// then `Vitest` share the `passed` keyword but disambiguate on the trailing punctuation. This
/// closes FD-8 (posx-first-external-use.md) -- gate result parsers were libtest-shaped, so a
/// configured Jest/Vitest gate could never publish a denominator.
pub fn unit_tests(stdout: &str, stderr: &str) -> D {
    // libtest: "N passed; M failed;" -- always on stdout.
    if let Some((p, f)) = before(stdout, " passed;").zip(before(stdout, " failed;")) {
        return D::Counted(p, p + f);
    }
    // Jest and Vitest print their summary to STDERR (verified against posx-frido-backend's
    // `npm run --silent test:unit`). Scan both so the parser does not depend on which stream a
    // runner happens to use.
    for stream in [stdout, stderr] {
        // Jest: `Tests:       N passed, T total`. Anchor on the `Tests:` prefix so a
        // `Test Suites:` line's own "N passed, M total" is not mistaken for the test count.
        if let Some(line) = stream
            .lines()
            .find(|l| l.trim_start().starts_with("Tests:") && l.contains(" total"))
        {
            if let Some((p, t)) = before(line, " passed,").zip(before(line, " total")) {
                return D::Counted(p, t);
            }
        }
        // Vitest: `Tests  N passed (T)`. Anchor on `Tests ` (with the whitespace, not a colon)
        // so the earlier `Test Files ` line's own `passed (` is not picked up.
        if let Some(line) = stream
            .lines()
            .find(|l| l.trim_start().starts_with("Tests ") && l.contains(" passed ("))
        {
            if let Some((p, t)) = before(line, " passed (").zip(after(line, " passed (")) {
                return D::Counted(p, t);
            }
        }
    }
    D::Unparseable
}
