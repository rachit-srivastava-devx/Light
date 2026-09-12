//! Learning probe: generates a scoped clarifying question from prior lessons.

use crate::port::{LessonHit, MemoryReader, ProbeError};

/// Input to the learning probe.
pub struct LearnInput {
    pub text: String,
    pub scope: String,
}

/// One question produced by the probe.
pub struct Question {
    pub text: String,
}

/// Probe `input` using `reader` for relevant prior lessons and return exactly one question.
///
/// Always returns `Ok(vec![..])` with one element — never an empty vec.
pub fn probe(input: &LearnInput, reader: &dyn MemoryReader) -> Result<Vec<Question>, ProbeError> {
    let hits = reader.recall(&input.text, &input.scope, 10).unwrap_or_default();
    let question_text = format_learning_prompt(&input.text, &input.scope, &hits);
    Ok(vec![Question { text: question_text }])
}

fn format_learning_prompt(text: &str, scope: &str, hits: &[LessonHit]) -> String {
    if hits.is_empty() {
        format!(
            "What lessons apply to '{}' in scope '{}'?",
            text.chars().take(40).collect::<String>(),
            scope
        )
    } else {
        let lesson_preview: String = hits[0].text.chars().take(50).collect();
        format!(
            "In scope '{}', prior learning: {}. What else applies to '{}'?",
            scope,
            lesson_preview,
            text.chars().take(40).collect::<String>()
        )
    }
}
