//! Byte-faithful port of `validate_atomic`'s row/tier/parent-chain rules (`intake.sh:203-227`).
//! Takes already-TSV-parsed rows (the caller does the `IFS=$'\t' read` and header check) -- this
//! fn owns only the row-shape, tier-composition, and design-decision-leakage decisions.

use super::atomic_types::{AtomicRow, AtomicTier};
use super::sow::StageViolation;
use std::collections::BTreeMap;

const DECISION_LEAK_WORDS: [&str; 9] = [
    "tbd", "todo", "to be decided", "decide", "design choice", "either", "unknown", "figure out",
    "determine",
];

fn leaks_decision(row: &AtomicRow) -> bool {
    let joined = format!("{} {} {} {}", row.description, row.inputs, row.outputs, row.acceptance).to_lowercase();
    DECISION_LEAK_WORDS.iter().any(|w| joined.contains(w))
}

/// Byte-faithful port of `validate_atomic_rows`.
pub fn validate_atomic_rows(rows: &[AtomicRow]) -> Vec<StageViolation> {
    let mut out = Vec::new();
    if rows.is_empty() {
        out.push(StageViolation("atomic decomposition has no rows".into()));
        return out;
    }
    let mut seen: BTreeMap<&str, AtomicTier> = BTreeMap::new();
    for row in rows {
        if seen.contains_key(row.id.as_str()) {
            out.push(StageViolation(format!("duplicate atomic id: {}", row.id)));
            continue;
        }
        seen.insert(row.id.as_str(), row.tier);
        if row.description.is_empty() || row.inputs.is_empty() || row.outputs.is_empty() || row.acceptance.is_empty() {
            out.push(StageViolation(format!("atomic row {} lacks an independent build contract", row.id)));
        }
        if row.design_decision != "none" && row.design_decision != "no" {
            out.push(StageViolation(format!("atomic leaf {} still contains a design decision: {}", row.id, row.design_decision)));
        } else if leaks_decision(row) {
            out.push(StageViolation(format!("atomic leaf {} still contains a design decision", row.id)));
        }
        let has_parents = !(row.parents.len() == 1 && row.parents[0] == "-");
        match row.tier {
            AtomicTier::Feature if has_parents => {
                out.push(StageViolation(format!("feature {} must have parents=-", row.id)));
            }
            AtomicTier::Service | AtomicTier::Module if !has_parents => {
                out.push(StageViolation(format!("{} {} must compose a lower tier", row.tier.name(), row.id)));
            }
            _ => {}
        }
    }
    for row in rows {
        check_parent_tiers(row, &seen, &mut out);
    }
    out
}

fn check_parent_tiers(row: &AtomicRow, seen: &BTreeMap<&str, AtomicTier>, out: &mut Vec<StageViolation>) {
    let required = match row.tier {
        AtomicTier::Service => Some(AtomicTier::Feature),
        AtomicTier::Module => Some(AtomicTier::Service),
        AtomicTier::Feature => None,
    };
    let Some(required) = required else { return };
    for parent in &row.parents {
        if parent == "-" {
            continue;
        }
        if seen.get(parent.as_str()) != Some(&required) {
            out.push(StageViolation(format!(
                "{} {} must compose {} {}",
                row.tier.name(), row.id, required.name(), parent
            )));
        }
    }
}
