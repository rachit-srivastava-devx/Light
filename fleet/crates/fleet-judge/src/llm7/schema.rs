//! Request body construction: an OpenAI-dialect chat-completion call with one forced tool,
//! `submit_verdict`, whose schema is the only channel the model has to answer -- no prose
//! parsing.

use crate::types::{Candidate, Criteria};
use crate::llm7::MODEL;
use serde_json::{json, Value};

pub fn build_request(criteria: &Criteria, candidate: &Candidate) -> Value {
    let system = format!(
        "You are a strict classifier judge. Instructions: {}\nAllowed labels: {:?}\n\
         Call submit_verdict exactly once. If none of the labels clearly fit, set \
         abstain_why instead of label; never guess.",
        criteria.instructions, criteria.labels
    );
    json!({
        "model": MODEL,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": candidate.input},
        ],
        "tools": [tool_schema(&criteria.labels)],
        "tool_choice": {"type": "function", "function": {"name": "submit_verdict"}},
    })
}

fn tool_schema(labels: &[String]) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "submit_verdict",
            "description": "Report the judge's decision or explicit abstention.",
            "parameters": {
                "type": "object",
                "properties": {
                    "label": {"type": "string", "enum": labels},
                    "confidence_pct": {"type": "integer", "minimum": 0, "maximum": 100},
                    "because": {"type": "string"},
                    "abstain_why": {"type": "string"},
                },
            },
        },
    })
}
