//! A repo's own unit-test suite -- the one gate parser that reads a target repo's test runner
//! output rather than one of fleet's own gate scripts, so it earns its own file: three summary
//! shapes to disambiguate, versus one fixed line format per fn in `parsers.rs`.

use super::super::denominator::DenominatorResult as D;
use super::super::digits::{after, before};

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
    if let Some(line) = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("test result:") && line.contains(" passed;"))
    {
        if let Some((p, f)) = before(line, " passed;").zip(before(line, " failed;")) {
            return D::Counted(p, p + f);
        }
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
