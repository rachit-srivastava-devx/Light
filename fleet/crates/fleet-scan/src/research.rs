//! `ResearchProbe`: uses `ResearchPort` to check an external claim the requirement makes. The
//! probe most likely to fault (§2/§6) -- no network, no API key, no result found.

use crate::input::{ProbeKind, RequirementInput};
use crate::ports::ResearchPort;
use crate::probe::{GapSeverity, Probe, ProbeOutcome, Question};

/// Uses `ResearchPort` to check an external claim the requirement makes (a named library, a
/// competitor behavior, a standard). The probe most likely to fault -- see §2/§6.
pub struct ResearchProbe<'a> {
    pub research: &'a dyn ResearchPort,
}

impl Probe for ResearchProbe<'_> {
    fn kind(&self) -> ProbeKind {
        ProbeKind::Research
    }

    fn probe(&self, input: &RequirementInput) -> ProbeOutcome {
        let results = match self.research.search(&input.text) {
            Ok(results) => results,
            Err(fault) => return ProbeOutcome::Fault(fault),
        };
        if results.is_empty() {
            return ProbeOutcome::Questions(vec![Question {
                probe: ProbeKind::Research,
                text: "No external reference found for this requirement -- is it well-known?"
                    .into(),
                why: "External research returned nothing to ground the requirement's claim"
                    .into(),
                gap: GapSeverity::Low,
                evidence: None,
            }]);
        }
        ProbeOutcome::Questions(Vec::new())
    }
}
