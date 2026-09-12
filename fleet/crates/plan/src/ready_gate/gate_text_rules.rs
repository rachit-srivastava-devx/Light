//! `contains_absolute_claim`/`matches_tautology`/`valid_revive_trigger`. Verbatim from
//! `lld_ready.rs:268-311`.

use super::gate_text::contains_word_ci;

/// `/\b(zero|never|impossible|cannot|no way)\b/i`.
pub(crate) fn contains_absolute_claim(claim: &str) -> bool {
    ["zero", "never", "impossible", "cannot", "no way"].iter().any(|word| contains_word_ci(claim, word))
}

/// The 5-pattern tautology denylist, hand-ported from `TAUTOLOGY_DENYLIST`. NOT trimmed first --
/// the TS check does not trim `a.then` either. The third pattern has no trailing anchor and is
/// therefore a PREFIX match, on purpose.
pub(crate) fn matches_tautology(then: &str) -> bool {
    let t = then.to_lowercase();
    if t == "work" || t == "works" || t == "it work" || t == "it works" {
        return true;
    }
    if t == "correct" || t == "is correct" {
        return true;
    }
    if t.starts_with("should work") || t.starts_with("it should work") {
        return true;
    }
    if t == "passes test" || t == "passes tests" || t == "passes the test" || t == "passes the tests" {
        return true;
    }
    if t == "no error" || t == "no errors" {
        return true;
    }
    false
}

/// `NON_TRIGGER_DENYLIST` plus the `/^never$/i` early-allow, exactly as `checkR21Alts` orders it.
pub(crate) fn valid_revive_trigger(trigger: &str) -> bool {
    let t = trigger.trim().to_lowercase();
    if t == "never" {
        return true;
    }
    !matches!(t.as_str(), "tbd" | "n/a" | "none" | "-" | "?")
}
