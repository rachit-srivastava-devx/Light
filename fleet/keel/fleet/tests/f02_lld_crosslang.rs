//! F02: the Rust leg of the cross-language `lld.v1` comparator (lane contract §7.5). Cases
//! F02-T6/T7/T9/T10/T11 restated in Rust's own idiom (the exact bodies are TS-specific and given
//! verbatim in the lane contract's §7.2; this file proves the same properties against the same
//! fixture corpus), plus the report emitter (F02-T12 equivalent, read by
//! `fleet/tests/acceptance/lld-crosslang.sh`) and the one Rust-only case, F02-T14, given verbatim
//! in the lane contract's §7.5.

use fleet::lld;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/fixtures/lld"
    ))
}

fn load(name: &str) -> Value {
    let text =
        fs::read_to_string(fixtures_dir().join(format!("{name}.json"))).expect("read fixture");
    serde_json::from_str(&text).expect("fixture must be valid JSON")
}

#[test]
fn f02_t6_rejects_a_leaf_grain_brief_with_only_one_killed_alternative() {
    let violations = lld::validate_module_brief(&load("one_alternative"));
    assert!(!violations.is_empty());
    assert!(
        violations.iter().any(|v| v.path == "alternatives"),
        "{violations:?}"
    );
}

#[test]
fn f02_t7_rejects_a_freeze_stamped_by_the_proposer() {
    let violations = lld::validate_lld_v1(&load("forged_freeze"));
    assert!(!violations.is_empty());
    assert!(
        violations.iter().any(|v| v.path == "freeze.stamped_by"),
        "{violations:?}"
    );
}

#[test]
fn f02_t9_canonical_json_refuses_a_numeric_leaf() {
    assert!(lld::canonical_json(&serde_json::json!({"ratio": 1.0})).is_err());
    let numeric_trap = load("numeric_trap");
    assert!(lld::content_hash(&numeric_trap).is_err());
    // and the guard is not vacuous: the good fixture still hashes.
    let complete_module = load("complete_module");
    let hash = lld::content_hash(&complete_module).expect("complete_module must hash");
    assert!(hash.starts_with("sha256:"));
    assert_eq!(hash.len(), "sha256:".len() + 64);
}

#[test]
fn f02_t10_content_hash_carries_its_algorithm() {
    let complete_module = load("complete_module");
    let hash = lld::content_hash(&complete_module).expect("complete_module must hash");
    assert!(hash.starts_with("sha256:"));
    assert_eq!(hash.len(), "sha256:".len() + 64);
    assert!(hash["sha256:".len()..]
        .bytes()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
}

#[test]
fn f02_t11_a_freeze_self_verifies_against_its_own_brief() {
    let wrapper = load("complete_freeze");
    let module_brief = wrapper.get("module_brief").expect("module_brief present");
    let recomputed = lld::content_hash(module_brief).expect("module_brief must hash");
    let stored = wrapper["freeze"]["content_hash"]
        .as_str()
        .expect("content_hash present");
    assert_eq!(stored, recomputed);
}

#[test]
fn f02_t12_emits_the_cross_language_report() {
    let report = lld::cross_lang_report(&fixtures_dir());
    let out = std::env::var("F02_REPORT_OUT").unwrap_or_else(|_| "/dev/null".to_string());
    // No trailing newline: matches the frozen TS test's actual behaviour
    // (`JSON.stringify(x, null, 2)`, which appends none) rather than this section's own prose in
    // the lane contract ("a trailing newline") -- see the F02 builder's final report.
    fs::write(
        out,
        serde_json::to_string_pretty(&report).expect("report must serialize"),
    )
    .expect("write report");
}

#[test]
fn f02_t14_no_source_path_constructs_the_stamper_literal() {
    // The const exists so a proposer cannot forge it. Assert the retirement, not just the
    // replacement: nothing in the crate may WRITE the string; only compare against it.
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lld.rs")).unwrap();
    let writes = src.matches("\"keel:lld-ready\"").count();
    let compares = src.matches("== \"keel:lld-ready\"").count() + src.matches("STAMPED_BY").count();
    assert!(writes > 0, "the const must exist");
    assert_eq!(
        writes, compares,
        "stamper literal is constructed, not only compared"
    );
}
