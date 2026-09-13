//! Research probe: resolves unknown terms via an injected `ResearchPort`.

use crate::port::{ResearchError, ResearchPort, Source};

/// Input carrying the requirement text and the unresolved term to research.
pub struct ResearchInput {
    pub text: String,
    pub unknown: String,
    /// Caller-supplied deadline passed directly to the port (milliseconds).
    pub deadline_ms: u64,
}

/// One clarifying question produced by the probe.
pub struct Question {
    pub text: String,
}

/// Combined output of a successful probe call.
pub struct ProbeResult {
    pub questions: Vec<Question>,
    pub sources: Vec<Source>,
}

/// Probe `input` via `port` and return one question and any retrieved sources.
///
/// On a `Timeout` error the probe returns a fallback question rather than propagating
/// the error, so the result is always `Ok(..)` with exactly one question.
pub fn probe(input: &ResearchInput, port: &dyn ResearchPort) -> Result<ProbeResult, ResearchError> {
    if input.text.is_empty() || input.unknown.is_empty() {
        return Err(ResearchError::EmptyQuery);
    }
    let query = format_query(&input.text, &input.unknown);
    match port.search(&query, input.deadline_ms) {
        Ok(sources) => {
            let text = question_from_sources(&sources, &input.unknown);
            Ok(ProbeResult {
                questions: vec![Question { text }],
                sources,
            })
        }
        Err(ResearchError::Timeout) => Ok(ProbeResult {
            questions: fallback_question(&input.unknown),
            sources: vec![],
        }),
        Err(e) => Err(e),
    }
}

fn format_query(text: &str, unknown: &str) -> String {
    format!("{text} — unknown: {unknown}")
}

fn question_from_sources(sources: &[Source], unknown: &str) -> String {
    let excerpt: String = sources
        .first()
        .map(|s| s.title.chars().take(80).collect::<String>())
        .unwrap_or_default();
    format!(
        "Based on research: {excerpt}. \
         What documentation and reference sources clarify: {unknown}?"
    )
}

fn fallback_question(unknown: &str) -> Vec<Question> {
    vec![Question {
        text: format!(
            "Unable to retrieve external documentation for '{unknown}'. \
             What local reference sources apply?"
        ),
    }]
}
