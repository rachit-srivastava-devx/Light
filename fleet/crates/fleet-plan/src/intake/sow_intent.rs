//! The `source_intent_hash` gate, split out of `sow.rs` to stay under the 80-line file cap.
//!
//! This crate opens no file. The old message ("does not match intent.txt (expected <flag>)")
//! named a file nothing here ever reads AND reported the caller's own flag as the expected
//! value, so a reader went looking for an `intent.txt` that does not exist and never saw what
//! the SOW itself declared. The gate compares exactly two in-memory strings: the SOW body's own
//! `source_intent_hash:` line, and the hash the caller computed (`intake.sh:50`'s `shasum -a
//! 256` subprocess, which is the composition root's job -- see BLUEPRINT §`validate_sow_text`).

/// The value of the SOW body's own `source_intent_hash:` line, if it declares one. Key match is
/// ASCII-case-insensitive; the value is trimmed. `None` means the body declares no such line.
fn declared_intent_hash(text: &str) -> Option<String> {
    text.lines()
        .find(|l| l.to_lowercase().starts_with("source_intent_hash:"))
        .map(|l| l.split_once(':').map(|x| x.1).unwrap_or("").trim().to_string())
}

/// `None` when the gate passes. Both sides are always named in the failure text, so a reader can
/// see which one to change. When NEITHER side carries a hash the gate is SKIPPED, not failed:
/// a caller with no recorded intent to pin to still gets the structural/content verdict, which
/// is the only reason to run `sow` at all. That is a refusal (exit 7) when it does fail, never
/// an environment fault (exit 3) -- there is no tool or file whose absence this can observe.
pub(crate) fn intent_hash_violation(sow_text: &str, intent_hash: &str) -> Option<String> {
    let declared = declared_intent_hash(sow_text);
    let supplied = intent_hash.trim();
    match (declared.as_deref(), supplied) {
        (None, "") => None,
        (Some(d), s) if d == s => None,
        (None, s) => Some(format!(
            "source_intent_hash: the SOW body declares no `source_intent_hash:` line, but \
             --intent-hash carried {s:?}. Add a line `source_intent_hash: {s}` to the SOW text \
             itself -- this check reads only the --text you passed, no intent.txt is read from \
             disk (this crate opens no file)."
        )),
        (Some(d), s) => Some(format!(
            "source_intent_hash mismatch: the SOW body declares {d:?}, --intent-hash carried \
             {s:?}. Both are compared as literal strings; nothing is read from disk."
        )),
    }
}
