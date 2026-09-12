//! Boundary and predicate-completeness tests for the materiality scan.

use fleet_scan::{scan, ScanDecision, ScanError, ScanInput, Unknown};

fn unknown(field: &str, missing_grant: bool, blocks_ready_node: bool) -> Unknown {
    Unknown {
        field: field.into(),
        alternatives: vec![],
        effect_changes: false,
        acceptance_changes: false,
        missing_grant,
        blocks_ready_node,
    }
}

fn effect_only(field: &str) -> Unknown {
    Unknown {
        field: field.into(),
        alternatives: vec![],
        effect_changes: true,
        acceptance_changes: false,
        missing_grant: false,
        blocks_ready_node: false,
    }
}

#[test]
fn acceptance_change_alone_is_material() {
    let u = Unknown {
        field: "api".into(),
        alternatives: vec![],
        effect_changes: false,
        acceptance_changes: true,
        missing_grant: false,
        blocks_ready_node: false,
    };
    let input = ScanInput { unknowns: vec![u], revision: 2 };
    assert!(matches!(scan(&input).unwrap(), ScanDecision::Probe { .. }));
}

#[test]
fn revision_preserved_in_probe() {
    let input = ScanInput { unknowns: vec![effect_only("field_b")], revision: 99 };
    match scan(&input).unwrap() {
        ScanDecision::Probe { revision, .. } => assert_eq!(revision, 99),
        _ => panic!("expected Probe"),
    }
}

#[test]
fn missing_grant_alone_is_material() {
    let input = ScanInput { unknowns: vec![unknown("grant", true, false)], revision: 5 };
    assert!(matches!(scan(&input).unwrap(), ScanDecision::Probe { .. }));
}

#[test]
fn blocks_ready_node_alone_is_material() {
    let input = ScanInput { unknowns: vec![unknown("blocker", false, true)], revision: 5 };
    assert!(matches!(scan(&input).unwrap(), ScanDecision::Probe { .. }));
}

#[test]
fn exactly_max_unknowns_is_fine() {
    let unknowns: Vec<Unknown> = (0..256)
        .map(|i| unknown(&format!("field_{i}"), false, false))
        .collect();
    let input = ScanInput { unknowns, revision: 1 };
    assert!(scan(&input).is_ok());
}

#[test]
fn over_max_unknowns_refused() {
    let unknowns: Vec<Unknown> = (0..257)
        .map(|i| unknown(&format!("field_{i}"), false, false))
        .collect();
    let input = ScanInput { unknowns, revision: 1 };
    assert!(matches!(scan(&input), Err(ScanError::TooManyUnknowns)));
}
