//! `evaluate`/`evaluate_with`/`depth_evidence`/`check_and_evaluate`. Verbatim from
//! `lld_ready.rs:107-204`.

use super::gate_checks_table::CHECKS;
use super::gate_types::{
    Check, DepthScore, GateReason, GateRefs, Outcome, Verdict, GATE_CHECK_IDS,
};
use super::module_brief::validate_module_brief;
use super::violation::Violation;
use serde_json::Value;

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

/// The `depth_evidence` block for `freeze.v1`. `None` for a `MeasuredNothing` verdict.
pub fn depth_evidence(v: &Verdict) -> Option<Value> {
    let score = v.score.as_ref()?;
    let checked_ids: Vec<Value> = GATE_CHECK_IDS
        .iter()
        .map(|id| Value::String((*id).to_string()))
        .collect();
    // ratio emitted as decimal for JSON consumers; ratio_milli/1000 keeps the field compatible.
    let ratio_json = serde_json::Number::from_f64(f64::from(score.ratio_milli) / 1000.0)
        .unwrap_or_else(|| serde_json::Number::from(0u32));
    Some(serde_json::json!({
        "score": {
            "checks_total": score.checks_total,
            "checks_passed": score.checks_passed,
            "ratio": ratio_json,
            "failed_check_ids": score.failed_check_ids,
        },
        "checked_check_ids": checked_ids,
    }))
}

/// Shape-then-readiness, as one pure function.
pub enum EntryOutcome {
    ShapeInvalid(Vec<Violation>),
    Gate(Verdict),
}

pub fn check_and_evaluate(brief: &Value, refs: &GateRefs) -> EntryOutcome {
    let violations = validate_module_brief(brief);
    if !violations.is_empty() {
        return EntryOutcome::ShapeInvalid(violations);
    }
    EntryOutcome::Gate(evaluate(brief, refs))
}
