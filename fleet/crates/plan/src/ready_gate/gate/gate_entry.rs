use super::super::gate_types::{GateRefs, Verdict};
use super::super::module_brief::validate_module_brief;
use super::super::violation::Violation;
use super::evaluate;
use serde_json::Value;

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
