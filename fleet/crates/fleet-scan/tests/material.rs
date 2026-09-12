//! Materiality gate contract tests — §10 named tests plus predicate variants.

use fleet_scan::{scan, ProbeKind, ScanDecision, ScanError, ScanInput, Unknown};

fn cosmetic() -> Unknown {
    Unknown {
        field: "field_a".into(),
        alternatives: vec![],
        effect_changes: false,
        acceptance_changes: false,
        missing_grant: false,
        blocks_ready_node: false,
    }
}

fn material_effect() -> Unknown {
    Unknown {
        field: "field_b".into(),
        alternatives: vec!["alt1".into()],
        effect_changes: true,
        acceptance_changes: false,
        missing_grant: false,
        blocks_ready_node: false,
    }
}

#[test]
fn cosmetic_unknown_yields_clear() {
    let input = ScanInput { unknowns: vec![cosmetic()], revision: 1 };
    let result = scan(&input).expect("scan must not error on valid input");
    assert_eq!(result, ScanDecision::Clear { revision: 1 });
}

#[test]
fn material_unknown_yields_probe() {
    let input = ScanInput { unknowns: vec![material_effect()], revision: 7 };
    let result = scan(&input).expect("scan must not error on valid input");
    match result {
        ScanDecision::Probe { kinds, revision } => {
            assert_eq!(revision, 7);
            assert_eq!(kinds.len(), 4);
            assert_eq!(kinds[0], ProbeKind::Business);
            assert_eq!(kinds[1], ProbeKind::Technical);
            assert_eq!(kinds[2], ProbeKind::Memory);
            assert_eq!(kinds[3], ProbeKind::Research);
        }
        ScanDecision::Clear { .. } => panic!("expected Probe, got Clear"),
    }
}

#[test]
fn duplicate_field_is_refused() {
    let mut u2 = cosmetic();
    u2.field = "field_a".into();
    let input = ScanInput { unknowns: vec![cosmetic(), u2], revision: 3 };
    let err = scan(&input).expect_err("duplicate field must be refused");
    assert!(matches!(err, ScanError::DuplicateField(f) if f == "field_a"));
}

#[test]
fn zero_revision_refused() {
    let input = ScanInput { unknowns: vec![], revision: 0 };
    assert!(matches!(scan(&input), Err(ScanError::EmptyRevision)));
}

#[test]
fn empty_unknowns_yields_clear() {
    let input = ScanInput { unknowns: vec![], revision: 42 };
    assert_eq!(scan(&input).unwrap(), ScanDecision::Clear { revision: 42 });
}

