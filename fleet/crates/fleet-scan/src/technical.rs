//! `TechnicalProbe`: uses `CodebasePort` to flag a requirement describing something as new that
//! already exists under some name in the current repo.

use crate::input::{ProbeKind, RequirementInput};
use crate::ports::CodebasePort;
use crate::probe::{GapSeverity, Probe, ProbeOutcome, Question};

/// Uses `CodebasePort` to check whether a named thing the requirement describes as new already
/// exists under some name in the current repo.
pub struct TechnicalProbe<'a> {
    pub codebase: &'a dyn CodebasePort,
}

/// Candidate identifier-shaped tokens: contain `_` or internal capitalization, length > 2.
fn identifier_candidates(text: &str) -> Vec<&str> {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '_'))
        .filter(|w| w.len() > 2)
        .filter(|w| w.contains('_') || w.chars().any(char::is_uppercase))
        .collect()
}

impl Probe for TechnicalProbe<'_> {
    fn kind(&self) -> ProbeKind {
        ProbeKind::Technical
    }

    fn probe(&self, input: &RequirementInput) -> ProbeOutcome {
        let mut questions = Vec::new();
        for name in identifier_candidates(&input.text) {
            match self.codebase.symbol_exists(name) {
                Ok(true) => questions.push(Question {
                    probe: ProbeKind::Technical,
                    text: format!("`{name}` already exists -- is this the same thing?"),
                    why: format!("`{name}` is already defined in the codebase under this name"),
                    gap: GapSeverity::Blocking,
                    evidence: Some(name.to_string()),
                }),
                Ok(false) => {}
                Err(fault) => return ProbeOutcome::Fault(fault),
            }
        }
        ProbeOutcome::Questions(questions)
    }
}
