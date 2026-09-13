//! `evaluate`/`evaluate_with`/`depth_evidence`/`check_and_evaluate`. Verbatim from
//! `lld_ready.rs:107-204`.

use super::gate_checks_table::CHECKS;
use super::gate_types::{Check, DepthScore, GateReason, GateRefs, Outcome, Verdict};
use serde_json::Value;

#[path = "gate_depth_evidence.rs"]
mod gate_depth_evidence;
#[path = "gate_entry.rs"]
mod gate_entry;
pub use gate_depth_evidence::depth_evidence;
pub use gate_entry::{check_and_evaluate, EntryOutcome};

/// The public entry point. Always measures the full vocabulary.
pub fn evaluate(brief: &Value, refs: &GateRefs) -> Verdict {
    evaluate_with(brief, refs, CHECKS)
}

/// The same evaluator over an explicit check slice. `pub` so an empty slice is reachable from a
/// test -- passing one is safe by construction: it yields a refusal, never a pass.
pub fn evaluate_with(brief: &Value, refs: &GateRefs, checks: &[Check]) -> Verdict {
    let checked = checks.len();
    if checked == 0 {
        return Verdict {
            outcome: Outcome::MeasuredNothing,
            checked: 0,
            score: None,
            reasons: Vec::new(),
        };
    }

    let mut reasons = Vec::new();
    for &(id, run) in checks {
        if !run(brief, refs) {
            reasons.push(GateReason {
                check_id: id,
                detail: format!("{id} failed"),
            });
        }
    }

    let checks_total = checked as u32;
    let checks_passed = checks_total - reasons.len() as u32;
    let ratio_milli = if checks_total > 0 {
        checks_passed * 1000 / checks_total
    } else {
        0
    };
    let failed_check_ids: Vec<String> = reasons.iter().map(|r| r.check_id.to_string()).collect();
    let score = DepthScore {
        checks_total,
        checks_passed,
        ratio_milli,
        failed_check_ids,
    };

    let outcome = if reasons.is_empty() {
        Outcome::Ready
    } else {
        Outcome::NotReady
    };
    Verdict {
        outcome,
        checked,
        score: Some(score),
        reasons,
    }
}

