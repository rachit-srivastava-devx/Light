use intent::{validate, Effect, IntentError, IntentInput, ModelProposal, PolicySnapshot};

fn proposal(kind: &str, goal: &str) -> ModelProposal {
    ModelProposal {
        kind: kind.into(),
        goal: goal.into(),
        effects: vec![],
        evidence: vec!["evidence-ref-1".into()],
        raw_json: serde_json::json!({}),
    }
}

fn input() -> IntentInput {
    IntentInput { request: "implement OAuth2 login".into(), context: String::new() }
}

fn policy() -> PolicySnapshot {
    PolicySnapshot { allowed_effects: vec![] }
}

#[test]
fn unknown_kind_refuses() {
    let result = validate(proposal("unknown-workflow", "do something"), &input(), &policy());
    assert!(matches!(result, Err(IntentError::UnknownKind(_))));
}

#[test]
fn extra_authority_field_refuses() {
    let mut p = proposal("feature", "implement login");
    p.raw_json = serde_json::json!({ "actor_id": "user-123" });
    let result = validate(p, &input(), &policy());
    assert!(matches!(result, Err(IntentError::AuthorityField(_))));
}

#[test]
fn empty_goal_refuses() {
    let result = validate(proposal("feature", ""), &input(), &policy());
    assert!(matches!(result, Err(IntentError::EmptyGoal)));
}

#[test]
fn valid_proposal_produces_intent_spec() {
    let result = validate(proposal("feature", "implement OAuth2 login"), &input(), &policy());
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    let spec = result.unwrap();
    assert!(!spec.kind.is_empty());
    assert!(!spec.goal.is_empty());
}

#[test]
fn missing_evidence_refuses() {
    let mut p = proposal("feature", "implement login");
    p.evidence = vec![];
    let result = validate(p, &input(), &policy());
    assert!(matches!(result, Err(IntentError::MissingEvidence(_))));
}
