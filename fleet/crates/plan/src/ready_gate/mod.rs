//! LLD-ready shape validator (`fleet/keel/fleet/src/lld.rs`) + depth gate
//! (`fleet/keel/fleet/src/lld_ready.rs`) -- lifted near-verbatim, split by section to stay under
//! the 80-line file cap.

mod accepts_when;
mod canonical;
mod freeze;
mod freeze_depth_evidence;
mod freeze_tail;
mod gate_checks_a;
mod gate_checks_b;
mod gate_checks_c;
mod gate_checks_table;
mod gate_eval;
mod gate_text;
mod gate_text_rules;
mod gate_types;
mod lld_v1;
mod module_brief;
mod module_brief_core;
mod module_brief_fields;
mod module_brief_mid;
mod module_brief_nested;
mod module_brief_nested2;
mod registry_verdict;
mod sow_seed;
mod violation;

pub use canonical::{canonical_json, content_hash, NumericLeafError};
pub use gate_checks_table::CHECKS;
pub use gate_eval::{check_and_evaluate, depth_evidence, evaluate, evaluate_with, EntryOutcome};
pub use gate_types::{Check, DepthScore, GateReason, GateRefs, Outcome, Verdict, GATE_CHECK_IDS, STAMPED_BY};
pub use lld_v1::validate_lld_v1;
pub use module_brief::validate_module_brief;
pub use violation::Violation;
