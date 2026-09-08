//! Splits each validated module summary's guarantees/open_questions/acceptance out into the
//! three flat, per-module walkthrough sections -- pulled out of `build.rs` to stay under 80 lines.

use super::extract::ModuleSummary;
use super::types::{AcceptancePreviewItem, DecisionItem, RiskItem};

pub(crate) fn key_decisions(summaries: &[ModuleSummary]) -> Vec<DecisionItem> {
    summaries
        .iter()
        .flat_map(|m| {
            m.guarantees
                .iter()
                .map(move |g| DecisionItem { node_id: m.node_id.clone(), claim: g.claim.clone(), label: g.label.clone() })
        })
        .collect()
}

pub(crate) fn risks(summaries: &[ModuleSummary]) -> Vec<RiskItem> {
    summaries
        .iter()
        .flat_map(|m| m.open_questions.iter().map(move |q| RiskItem { node_id: m.node_id.clone(), risk: q.clone() }))
        .collect()
}

pub(crate) fn acceptance_preview(summaries: &[ModuleSummary]) -> Vec<AcceptancePreviewItem> {
    summaries
        .iter()
        .filter_map(|m| {
            m.acceptance.as_ref().map(|a| AcceptancePreviewItem {
                node_id: m.node_id.clone(),
                given: a.given.clone(),
                when: a.when.clone(),
                then: a.then.clone(),
                oracle_kind: a.oracle_kind.clone(),
            })
        })
        .collect()
}

/// Ranked, sorted node_ids with open questions; if none, the first module in build order.
pub(crate) fn owner_focus(summaries: &[ModuleSummary], fallback: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = summaries.iter().filter(|m| !m.open_questions.is_empty()).map(|m| m.node_id.clone()).collect();
    out.sort();
    if out.is_empty() {
        out = fallback.map(|id| vec![id.to_string()]).unwrap_or_default();
    }
    out
}
