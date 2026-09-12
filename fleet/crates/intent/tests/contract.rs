use intent::{classify, IntentError, IntentSpec, WorkflowSelection};

fn spec(kind: &str, desc: &str) -> IntentSpec {
    IntentSpec { kind: kind.into(), description: desc.into(), constraints: vec![], authority: vec![] }
}

#[test]
fn unknown_kind_refuses() {
    let result = classify(&spec("unknown", "do something"));
    assert!(matches!(result, Err(IntentError::UnknownKind(_))));
}

#[test]
fn disagreement_chooses_conservative() {
    let result = classify(&spec("fix", "fix the bug")).unwrap();
    assert_eq!(result, WorkflowSelection::FastFix);
}

#[test]
fn extra_authority_field_refuses() {
    let result = classify(&spec("feature", ""));
    assert!(matches!(result, Err(IntentError::EmptyDescription)));
}
