use crate::gate::independent_kind;
use crate::{IntentError, IntentInput, IntentSpec, ModelProposal, PolicySnapshot};

const ALLOWED_KINDS: &[&str] = &[
    "answer",
    "investigate",
    "review-only",
    "small-change",
    "feature",
    "refactor",
    "incident",
    "multi-repo-change",
    "research-design",
];

const AUTHORITY_FIELDS: &[&str] = &[
    "actor_id",
    "approval",
    "settlement",
    "authorization",
    "time",
];

pub fn validate(
    proposal: ModelProposal,
    input: &IntentInput,
    _policy: &PolicySnapshot,
) -> Result<IntentSpec, IntentError> {
    if !ALLOWED_KINDS.contains(&proposal.kind.as_str()) {
        return Err(IntentError::UnknownKind(proposal.kind));
    }
    if let serde_json::Value::Object(map) = &proposal.raw_json {
        for field in AUTHORITY_FIELDS {
            if map.contains_key(*field) {
                return Err(IntentError::AuthorityField((*field).to_string()));
            }
        }
    }
    if proposal.goal.is_empty() {
        return Err(IntentError::EmptyGoal);
    }
    if proposal.evidence.is_empty() {
        return Err(IntentError::MissingEvidence("evidence".to_string()));
    }
    let det = independent_kind(&input.request, &proposal.effects);
    let kind = conservative(&proposal.kind, &det);
    Ok(IntentSpec {
        kind,
        goal: proposal.goal,
        effects: proposal.effects,
        evidence: proposal.evidence,
    })
}

fn conservative(model: &str, det: &str) -> String {
    let rank = |k: &str| -> u8 {
        match k {
            "answer" => 0,
            "review-only" => 1,
            "investigate" => 2,
            "small-change" => 3,
            "research-design" => 4,
            "refactor" => 5,
            "feature" => 6,
            "incident" => 7,
            "multi-repo-change" => 8,
            _ => 9,
        }
    };
    if rank(det) <= rank(model) {
        det.to_string()
    } else {
        model.to_string()
    }
}
