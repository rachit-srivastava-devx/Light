//! Business probe: generates a clarifying question for a requirement input.

use crate::port::{BusinessReader, ProbeError};

/// The requirement text to probe.
pub struct BusinessInput {
    pub text: String,
}

/// One clarifying question produced by the probe.
pub struct Question {
    pub text: String,
}

const DOMAIN_TERMS: &[&str] = &[
    "stakeholder",
    "requirement",
    "constraint",
    "scope",
    "budget",
];

/// Probe `input` using `reader` for additional context and return exactly one question.
///
/// Always returns `Ok(vec![..])` with one element — never an empty vec.
pub fn probe(
    input: &BusinessInput,
    reader: &dyn BusinessReader,
) -> Result<Vec<Question>, ProbeError> {
    let context = reader.recall(&input.text).unwrap_or_default();
    let q = build_question(&input.text, &context);
    Ok(vec![Question { text: q }])
}

fn build_question(input_text: &str, context: &str) -> String {
    let combined = format!("{} {}", input_text, context).to_lowercase();
    let term = DOMAIN_TERMS
        .iter()
        .find(|&&t| combined.contains(t))
        .copied()
        .unwrap_or("requirement");
    if input_text.trim().is_empty() {
        format!("What are the key {}s for this task?", term)
    } else {
        let excerpt: String = input_text.chars().take(80).collect();
        format!("What are the {}s and constraints for: {}?", term, excerpt)
    }
}
