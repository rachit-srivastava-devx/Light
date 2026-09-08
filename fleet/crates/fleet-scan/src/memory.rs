//! `MemoryProbe`: uses `MemoryPort` to recall prior requirements/decisions textually similar to
//! this one and flag apparent contradiction/duplication.

use crate::input::{ProbeKind, RequirementInput};
use crate::ports::MemoryPort;
use crate::probe::{GapSeverity, Probe, ProbeOutcome, Question};

const RECALL_LIMIT: usize = 3;
const SIMILARITY_FLAG_THRESHOLD: f32 = 0.75;

/// Uses `MemoryPort` to recall prior requirements/decisions textually similar to this one and
/// flag apparent contradiction/duplication.
pub struct MemoryProbe<'a> {
    pub memory: &'a dyn MemoryPort,
}

impl Probe for MemoryProbe<'_> {
    fn kind(&self) -> ProbeKind {
        ProbeKind::Memory
    }

    fn probe(&self, input: &RequirementInput) -> ProbeOutcome {
        let hits = match self.memory.recall_similar(&input.text, RECALL_LIMIT) {
            Ok(hits) => hits,
            Err(fault) => return ProbeOutcome::Fault(fault),
        };
        let questions = hits
            .into_iter()
            .filter(|hit| hit.score > SIMILARITY_FLAG_THRESHOLD)
            .map(|hit| Question {
                probe: ProbeKind::Memory,
                text: "This looks like a prior decision -- which one governs?".into(),
                why: format!("Similar to a remembered item: \"{}\"", hit.text),
                gap: GapSeverity::High,
                evidence: Some(hit.text),
            })
            .collect();
        ProbeOutcome::Questions(questions)
    }
}
