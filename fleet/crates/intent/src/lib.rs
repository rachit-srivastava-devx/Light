pub mod gate;
pub mod prompt;
pub mod schema;

pub use gate::independent_kind;
pub use prompt::IntentModel;
pub use schema::validate;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Effect { pub kind: String }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IntentInput { pub request: String, pub context: String }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PolicySnapshot { pub allowed_effects: Vec<Effect> }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ModelProposal {
    pub kind: String,
    pub goal: String,
    pub effects: Vec<Effect>,
    pub evidence: Vec<String>,
    pub raw_json: serde_json::Value,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IntentSpec {
    pub kind: String,
    pub goal: String,
    pub effects: Vec<Effect>,
    pub evidence: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum IntentError {
    #[error("unknown workflow kind: {0}")]
    UnknownKind(String),
    #[error("authority field rejected: {0}")]
    AuthorityField(String),
    #[error("missing evidence for: {0}")]
    MissingEvidence(String),
    #[error("goal is empty")]
    EmptyGoal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn p(kind: &str, raw: serde_json::Value) -> ModelProposal {
        ModelProposal { kind: kind.into(), goal: "g".into(),
            effects: vec![], evidence: vec!["e".into()], raw_json: raw }
    }
    fn i(req: &str) -> IntentInput {
        IntentInput { request: req.into(), context: String::new() }
    }
    fn pol() -> PolicySnapshot { PolicySnapshot { allowed_effects: vec![] } }
    #[test]
    fn unknown_kind_refuses() {
        let r = validate(p("deploy-infrastructure",
            json!({"kind":"deploy-infrastructure"})), &i(""), &pol());
        assert!(matches!(r, Err(IntentError::UnknownKind(_))));
    }

    #[test]
    fn disagreement_chooses_conservative() {
        let spec = validate(p("feature", json!({"kind":"feature"})),
            &i("fix a small bug"), &pol()).unwrap();
        assert_eq!(spec.kind, "small-change");
    }

    #[test]
    fn extra_authority_field_refuses() {
        let r = validate(p("feature",
            json!({"kind":"feature","actor_id":"u1"})), &i(""), &pol());
        assert!(matches!(r, Err(IntentError::AuthorityField(_))));
    }
}
