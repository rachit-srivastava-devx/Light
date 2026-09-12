use fleet_plan::{check_and_evaluate, depth_evidence, evaluate, evaluate_with, EntryOutcome, GateRefs, Outcome};
use serde_json::json;

fn complete_refs() -> GateRefs {
    GateRefs {
        owners: vec!["rachit@devxlabs.ai".to_string()],
        registry_paths: vec!["registry/services/llm-gateway".to_string()],
        known_node_ids: vec!["orb-freeze-ledger".to_string()],
    }
}

#[test]
fn evaluate_is_total_over_a_completely_empty_object() {
    let verdict = evaluate(&json!({}), &complete_refs());
    assert_eq!(verdict.outcome, Outcome::NotReady);
    assert_eq!(verdict.checked, 14);
}

#[test]
fn evaluate_with_empty_checks_measures_nothing() {
    let verdict = evaluate_with(&json!({}), &complete_refs(), &[]);
    assert_eq!(verdict.outcome, Outcome::MeasuredNothing);
    assert!(verdict.score.is_none());
}

#[test]
fn depth_evidence_is_none_for_measured_nothing() {
    let verdict = evaluate_with(&json!({}), &complete_refs(), &[]);
    assert!(depth_evidence(&verdict).is_none());
}

#[test]
fn depth_evidence_reports_the_full_check_vocabulary() {
    let verdict = evaluate(&json!({}), &complete_refs());
    let ev = depth_evidence(&verdict).unwrap();
    assert_eq!(ev["checked_check_ids"].as_array().unwrap().len(), 14);
}

#[test]
fn check_and_evaluate_reports_shape_invalid_before_running_the_gate() {
    match check_and_evaluate(&json!({"schema_version": "2.0"}), &complete_refs()) {
        EntryOutcome::ShapeInvalid(v) => assert!(!v.is_empty()),
        EntryOutcome::Gate(_) => panic!("expected shape-invalid"),
    }
}
