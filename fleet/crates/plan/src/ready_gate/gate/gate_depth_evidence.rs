use super::super::gate_types::{Verdict, GATE_CHECK_IDS};
use serde_json::Value;

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
