//! `judge` -- pure orchestration over ONE `JudgeModel` call, in the shape of
//! `fleet-memory::gate_check::check_added_line`. No I/O, no clock; every input is caller-
//! supplied and every output path is exhaustively typed.

use crate::errors::JudgeError;
use crate::model::JudgeModel;
use crate::types::{Candidate, Criteria, RawVerdict};
use crate::verdict::Verdict;

/// Judge `candidate` against `criteria` via one call through `model`. Never fabricates a
/// verdict: a model reply that is not a clean decision or a clean abstention becomes
/// `JudgeError::MalformedResponse`, not a default label.
pub fn judge(
    criteria: &Criteria,
    candidate: &Candidate,
    model: &dyn JudgeModel,
) -> Result<Verdict, JudgeError> {
    if criteria.labels.is_empty() {
        return Err(JudgeError::EmptyLabelSet);
    }
    let raw = model.call(criteria, candidate)?;
    to_verdict(criteria, raw)
}

fn to_verdict(criteria: &Criteria, raw: RawVerdict) -> Result<Verdict, JudgeError> {
    if let Some(why) = raw.abstain_why {
        if !why.trim().is_empty() {
            return Ok(Verdict::Abstain { why });
        }
        return Err(JudgeError::MalformedResponse(
            "abstain_why present but empty".into(),
        ));
    }
    let label = raw
        .label
        .ok_or_else(|| JudgeError::MalformedResponse("no label and no abstain_why".into()))?;
    if !criteria.labels.iter().any(|l| l == &label) {
        return Err(JudgeError::MalformedResponse(format!(
            "label {label:?} is not one of {:?}",
            criteria.labels
        )));
    }
    let confidence_pct = raw
        .confidence_pct
        .ok_or_else(|| JudgeError::MalformedResponse("decided but no confidence_pct".into()))?;
    if confidence_pct > 100 {
        return Err(JudgeError::MalformedResponse(format!(
            "confidence_pct {confidence_pct} out of range 0..=100"
        )));
    }
    let because = raw
        .because
        .filter(|b| !b.trim().is_empty())
        .ok_or_else(|| JudgeError::MalformedResponse("decided but no because rationale".into()))?;
    Ok(Verdict::Decided {
        label,
        confidence_pct,
        because,
    })
}
