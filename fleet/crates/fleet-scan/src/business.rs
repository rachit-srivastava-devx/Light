//! `BusinessProbe`: heuristic text checks for missing business-outcome signal. No port -- pure
//! text analysis over `RequirementInput.text`, greenfield (no fleet source to reuse from).

use crate::input::{ProbeKind, RequirementInput};
use crate::probe::{GapSeverity, Probe, ProbeOutcome, Question};

const METRIC_WORDS: &[&str] = &["metric", "kpi", "success", "measure", "target", "goal"];
const AUDIENCE_WORDS: &[&str] = &["user", "customer", "team", "client", "audience", "stakeholder"];

/// Reads `RequirementInput.text` for missing business-outcome signal (no stated success metric,
/// no stated audience). Pure heuristic text analysis -- no port, greenfield (§5/§7).
pub struct BusinessProbe;

fn contains_any(text: &str, words: &[&str]) -> bool {
    let lower = text.to_lowercase();
    words.iter().any(|w| lower.contains(w))
}

impl Probe for BusinessProbe {
    fn kind(&self) -> ProbeKind {
        ProbeKind::Business
    }

    fn probe(&self, input: &RequirementInput) -> ProbeOutcome {
        let mut questions = Vec::new();
        if !contains_any(&input.text, METRIC_WORDS) {
            questions.push(Question {
                probe: ProbeKind::Business,
                text: "What defines success for this requirement?".into(),
                why: "No stated success metric -- work could ship and still miss the goal".into(),
                gap: GapSeverity::High,
                evidence: None,
            });
        }
        if !contains_any(&input.text, AUDIENCE_WORDS) {
            questions.push(Question {
                probe: ProbeKind::Business,
                text: "Who is this requirement for?".into(),
                why: "No stated audience -- scope and priority are unclear without one".into(),
                gap: GapSeverity::Medium,
                evidence: None,
            });
        }
        ProbeOutcome::Questions(questions)
    }
}
