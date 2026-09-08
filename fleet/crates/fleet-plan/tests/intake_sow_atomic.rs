use fleet_plan::{validate_atomic_rows, validate_sow_text, AtomicRow, AtomicTier};

fn sow_body() -> String {
    "source_intent_hash: abc123\n\
## Request restatement\nbuild the thing\n\
## Built for\nusers\n\
## Must do\nthe thing\n\
## Explicitly will not do\nnot delete data, out of scope: billing\n\
## Done when\n95% pass\n\
## Acceptance threshold\nexit 0\n\
request: build the thing\n"
        .to_string()
}

#[test]
fn validate_sow_text_accepts_a_well_formed_sow() {
    let v = validate_sow_text(&sow_body(), "abc123");
    assert!(v.is_empty(), "{v:?}");
}

#[test]
fn validate_sow_text_rejects_hash_mismatch() {
    let v = validate_sow_text(&sow_body(), "different");
    assert!(v.iter().any(|e| e.0.contains("source_intent_hash")));
}

#[test]
fn validate_sow_text_empty_is_missing() {
    let v = validate_sow_text("", "abc123");
    assert_eq!(v.len(), 1);
    assert!(v[0].0.contains("missing or empty"));
}

fn atomic_row(id: &str, tier: AtomicTier, parents: &[&str]) -> AtomicRow {
    AtomicRow {
        id: id.to_string(),
        tier,
        parents: parents.iter().map(|s| s.to_string()).collect(),
        description: "d".into(),
        inputs: "i".into(),
        outputs: "o".into(),
        acceptance: "a".into(),
        design_decision: "none".into(),
    }
}

#[test]
fn atomic_row_rejects_wrong_parent_tier() {
    let rows = vec![
        atomic_row("f1", AtomicTier::Feature, &["-"]),
        atomic_row("m1", AtomicTier::Module, &["f1"]),
    ];
    let v = validate_atomic_rows(&rows);
    assert!(v.iter().any(|e| e.0.contains("must compose service")));
}

#[test]
fn atomic_row_accepts_valid_chain() {
    let rows = vec![
        atomic_row("f1", AtomicTier::Feature, &["-"]),
        atomic_row("s1", AtomicTier::Service, &["f1"]),
        atomic_row("m1", AtomicTier::Module, &["s1"]),
    ];
    let v = validate_atomic_rows(&rows);
    assert!(v.is_empty(), "{v:?}");
}

#[test]
fn atomic_rejects_duplicate_id() {
    let rows = vec![atomic_row("f1", AtomicTier::Feature, &["-"]), atomic_row("f1", AtomicTier::Feature, &["-"])];
    let v = validate_atomic_rows(&rows);
    assert!(v.iter().any(|e| e.0.contains("duplicate atomic id")));
}
