use crate::types::{ContextError, Evidence, EvidenceRef, Span, TokenCounter};

/// Pack mandatory spans first, then evidence items, up to (budget - reserve) tokens.
/// Returns (packed evidence refs, omitted labels).
pub fn pack(
    mandatory: &[Span],
    candidates: &[Evidence],
    budget: u64,
    reserve: u64,
    counter: &dyn TokenCounter,
) -> Result<(Vec<EvidenceRef>, Vec<String>), ContextError> {
    let available = budget
        .checked_sub(reserve)
        .ok_or(ContextError::BudgetExceeded)?;
    // mandatory tokens are always included; reject if they alone overflow
    let mandatory_tokens: u64 = mandatory.iter().map(|s| counter.count(&s.label)).sum();
    if mandatory_tokens > available {
        return Err(ContextError::BudgetExceeded);
    }
    let mut used = mandatory_tokens;
    let mut refs = Vec::new();
    let mut omitted = Vec::new();
    // dedup by source_digest
    let mut seen = std::collections::HashSet::new();
    for ev in candidates {
        if ev.source_digest.is_empty() {
            return Err(ContextError::MissingProvenance);
        }
        if !seen.insert(ev.source_digest.clone()) {
            continue; // dedup
        }
        let tokens = counter.count(&ev.content);
        let new_used = used
            .checked_add(tokens)
            .ok_or(ContextError::BudgetExceeded)?;
        if new_used <= available {
            used = new_used;
            refs.push(EvidenceRef {
                source_digest: ev.source_digest.clone(),
                label: ev.content[..ev.content.len().min(80)].to_string(),
                tokens,
            });
        } else {
            omitted.push(ev.source_digest.clone());
        }
    }
    Ok((refs, omitted))
}
