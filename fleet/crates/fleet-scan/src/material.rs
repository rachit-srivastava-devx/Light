//! Four-term materiality predicate and bounded scan decision.

use std::collections::HashSet;

use crate::{ProbeKind, ScanDecision, ScanError, ScanInput, Unknown};

const MAX_UNKNOWNS: usize = 256;

/// An unknown is material iff at least one of the four LLD terms is true.
pub fn is_material(u: &Unknown) -> bool {
    u.effect_changes || u.acceptance_changes || u.missing_grant || u.blocks_ready_node
}

/// Run the materiality scan: returns Clear when no unknown is material,
/// or Probe with the fixed four probe kinds when at least one is material.
pub fn scan(input: &ScanInput) -> Result<ScanDecision, ScanError> {
    if input.revision == 0 {
        return Err(ScanError::EmptyRevision);
    }
    if input.unknowns.len() > MAX_UNKNOWNS {
        return Err(ScanError::TooManyUnknowns);
    }

    let mut seen: HashSet<&str> = HashSet::new();
    for u in &input.unknowns {
        if !seen.insert(u.field.as_str()) {
            return Err(ScanError::DuplicateField(u.field.clone()));
        }
    }

    let has_material = input.unknowns.iter().any(is_material);

    if !has_material {
        return Ok(ScanDecision::Clear { revision: input.revision });
    }

    // Deterministic canonical order; exactly the fixed four kinds.
    let kinds = vec![
        ProbeKind::Business,
        ProbeKind::Technical,
        ProbeKind::Memory,
        ProbeKind::Research,
    ];

    Ok(ScanDecision::Probe { kinds, revision: input.revision })
}
