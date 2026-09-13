use super::super::atomic_types::{AtomicRow, AtomicTier};
use super::super::sow::StageViolation;
use std::collections::BTreeMap;

pub(super) fn check_parent_tiers(
    row: &AtomicRow,
    seen: &BTreeMap<&str, AtomicTier>,
    out: &mut Vec<StageViolation>,
) {
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
                row.tier.name(),
                row.id,
                required.name(),
                parent
            )));
        }
    }
}
