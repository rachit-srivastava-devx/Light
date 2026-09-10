//! The `source_intent_hash` gate. It used to report "does not match intent.txt (expected <the
//! caller's own flag>)": it named a file this crate never opens, and printed the flag as though
//! it were the computed expectation, so the caller created an `intent.txt`, hashed it, and was
//! refused again. The gate compares the SOW body's own declared line against the supplied hash,
//! and the message must name BOTH sides.

use fleet_plan::validate_sow_text;

fn sow(hash_line: &str) -> String {
    format!(
        "{hash_line}\
request: build the thing\n\
## Request restatement\nbuild the thing\n\
## Built for\nusers\n\
## Must do\nthe thing\n\
## Explicitly will not do\nnot delete data, out of scope: billing\n\
## Done when\n95% pass\n\
## Acceptance threshold\nexit 0\n"
    )
}

fn hash_reason(v: &[fleet_plan::StageViolation]) -> Option<String> {
    v.iter().find(|e| e.0.contains("source_intent_hash")).map(|e| e.0.clone())
}

#[test]
fn a_mismatch_names_both_sides_and_no_file() {
    let v = validate_sow_text(&sow("source_intent_hash: abc123\n"), "deadbeef");
    let reason = hash_reason(&v).expect("a mismatch must be reported");
    assert!(reason.contains("abc123"), "must report what the SOW declared: {reason}");
    assert!(reason.contains("deadbeef"), "must report what the flag carried: {reason}");
    assert!(!reason.contains("intent.txt"), "must not name a file nothing reads: {reason}");
}

#[test]
fn a_body_with_no_hash_line_is_told_to_add_one() {
    let v = validate_sow_text(&sow(""), "deadbeef");
    let reason = hash_reason(&v).expect("a supplied hash with nothing to match must be reported");
    assert!(reason.contains("declares no"), "{reason}");
    assert!(reason.contains("deadbeef"), "must echo the flag it could not match: {reason}");
}

#[test]
fn the_gate_is_skipped_when_neither_side_carries_a_hash() {
    // A caller with no recorded intent still gets the structural verdict rather than an
    // unpassable gate: nothing declared, nothing supplied, nothing to compare.
    let v = validate_sow_text(&sow(""), "");
    assert!(v.is_empty(), "an unpinned but well-formed SOW must pass, got {v:?}");
}

#[test]
fn a_matching_hash_passes() {
    let v = validate_sow_text(&sow("source_intent_hash: abc123\n"), "abc123");
    assert!(v.is_empty(), "{v:?}");
    let v = validate_sow_text(&sow("SOURCE_INTENT_HASH:   abc123  \n"), "  abc123 ");
    assert!(v.is_empty(), "key is case-insensitive and both sides are trimmed: {v:?}");
}
