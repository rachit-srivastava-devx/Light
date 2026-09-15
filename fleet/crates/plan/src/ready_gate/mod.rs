//! LLD-ready shape validator (`fleet/keel/fleet/src/lld.rs`) + depth gate
//! (`fleet/keel/fleet/src/lld_ready.rs`) -- lifted near-verbatim, split by section to stay under
//! the 80-line file cap.

#[path = "checks/accepts_when.rs"]
mod accepts_when;
mod canonical;
#[path = "freeze/freeze.rs"]
mod freeze;
#[path = "freeze/freeze_rest.rs"]
mod freeze_rest;
#[path = "freeze/freeze_tail.rs"]
mod freeze_tail;
#[path = "checks/gate_checks_a.rs"]
mod gate_checks_a;
#[path = "checks/gate_checks_b.rs"]
mod gate_checks_b;
#[path = "checks/gate_checks_c.rs"]
mod gate_checks_c;
#[path = "checks/gate_checks_table.rs"]
mod gate_checks_table;
#[path = "gate/gate_eval.rs"]
mod gate_eval;
#[path = "checks/gate_text.rs"]
mod gate_text;
#[path = "checks/gate_text_rules.rs"]
mod gate_text_rules;
#[path = "gate/gate_types.rs"]
mod gate_types;
mod lld_v1;
#[path = "brief/module_brief.rs"]
mod module_brief;
#[path = "brief/module_brief_alternatives.rs"]
mod module_brief_alternatives;
#[path = "brief/module_brief_core.rs"]
mod module_brief_core;
#[path = "brief/module_brief_fields.rs"]
mod module_brief_fields;
#[path = "brief/module_brief_mid.rs"]
mod module_brief_mid;
#[path = "brief/module_brief_nested.rs"]
mod module_brief_nested;
mod registry_verdict;
mod sow_seed;
mod violation;

pub use canonical::{canonical_json, content_hash, NumericLeafError};
pub use gate_checks_table::CHECKS;
pub use gate_eval::{check_and_evaluate, depth_evidence, evaluate, evaluate_with, EntryOutcome};
pub use gate_types::{
    Check, DepthScore, GateReason, GateRefs, Outcome, Verdict, GATE_CHECK_IDS, STAMPED_BY,
};
pub use lld_v1::validate_lld_v1;
pub use module_brief::validate_module_brief;
pub use violation::Violation;
