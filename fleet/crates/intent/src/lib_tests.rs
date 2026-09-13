use super::*;
use serde_json::json;

fn p(kind: &str, raw: serde_json::Value) -> ModelProposal {
    ModelProposal {
        kind: kind.into(),
        goal: "g".into(),
        effects: vec![],
        evidence: vec!["e".into()],
        raw_json: raw,
    }
}
fn i(req: &str) -> IntentInput {
    IntentInput {
        request: req.into(),
        context: String::new(),
    }
}
fn pol() -> PolicySnapshot {
    PolicySnapshot {
        allowed_effects: vec![],
    }
}
#[test]
fn unknown_kind_refuses() {
    let r = validate(
        p(
            "deploy-infrastructure",
            json!({"kind":"deploy-infrastructure"}),
        ),
        &i(""),
        &pol(),
    );
    assert!(matches!(r, Err(IntentError::UnknownKind(_))));
}

#[test]
fn disagreement_chooses_conservative() {
    let spec = validate(
        p("feature", json!({"kind":"feature"})),
        &i("fix a small bug"),
        &pol(),
    )
    .unwrap();
    assert_eq!(spec.kind, "small-change");
}

#[test]
fn extra_authority_field_refuses() {
    let r = validate(
        p("feature", json!({"kind":"feature","actor_id":"u1"})),
        &i(""),
        &pol(),
    );
    assert!(matches!(r, Err(IntentError::AuthorityField(_))));
}
