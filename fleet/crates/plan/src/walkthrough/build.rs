//! `build_walkthrough` -- the deterministic control-plane assembly for ask #6 ("teach the user
//! on the implementation suggested, walk him through it"). Pure: every fact comes from the
//! caller's already-`validate_module_brief`-shaped module briefs, no IO/clock/RNG/model call.

use super::error::WalkthroughError;
use super::extract::extract_module_summary;
use super::sections::{acceptance_preview, key_decisions, owner_focus, risks};
use super::types::{BuildItem, Walkthrough, WorkOrderItem};
use super::work_order::compute_work_order;
use crate::ready_gate::validate_module_brief;
use serde_json::Value;
use std::collections::BTreeSet;

/// `module_briefs` is the plan's module briefs (LLD leaves) in any order; this crate has already
/// validated each one's shape via `validate_module_brief`, re-run here so a walkthrough can never
/// narrate a brief this crate would otherwise reject.
pub fn build_walkthrough(title: &str, module_briefs: &[Value]) -> Result<Walkthrough, WalkthroughError> {
    if module_briefs.is_empty() {
        return Err(WalkthroughError::EmptyPlan);
    }

    let mut summaries = Vec::with_capacity(module_briefs.len());
    let mut seen = BTreeSet::new();
    for (index, brief) in module_briefs.iter().enumerate() {
        let violations = validate_module_brief(brief);
        if !violations.is_empty() {
            return Err(WalkthroughError::InvalidModuleBrief { index, violations });
        }
        let summary = extract_module_summary(brief).ok_or(WalkthroughError::IncompleteModuleBrief { index })?;
        if !seen.insert(summary.node_id.clone()) {
            return Err(WalkthroughError::DuplicateNodeId(summary.node_id));
        }
        summaries.push(summary);
    }

    let what_will_be_built: Vec<BuildItem> = summaries
        .iter()
        .map(|m| BuildItem { node_id: m.node_id.clone(), purpose: m.purpose.clone() })
        .collect();

    let steps = compute_work_order(&summaries).map_err(|cycle| WalkthroughError::CyclicDependency { cycle })?;
    let work_order = steps
        .into_iter()
        .enumerate()
        .map(|(i, s)| WorkOrderItem { position: i as u32 + 1, node_id: s.node_id, because: s.because })
        .collect();

    let fallback = what_will_be_built.first().map(|b| b.node_id.as_str());
    Ok(Walkthrough {
        title: title.to_string(),
        key_decisions: key_decisions(&summaries),
        risks: risks(&summaries),
        acceptance_preview: acceptance_preview(&summaries),
        owner_focus: owner_focus(&summaries, fallback),
        what_will_be_built,
        work_order,
    })
}
