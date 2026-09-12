use crate::clock::Clock;
use crate::{KnowledgeError, KnowledgeItem, KnowledgeStore, Scope, SourceManifest};

/// Returns items whose scope matches exactly.
pub fn filter_by_scope<'a>(items: &'a [KnowledgeItem], scope: &Scope) -> Vec<&'a KnowledgeItem> {
    items.iter().filter(|i| &i.scope == scope).collect()
}

/// Returns `true` if the item has an `expires_at` that is in the past.
pub fn check_expiry(item: &KnowledgeItem, clock: &dyn Clock) -> bool {
    let now = clock.now_iso();
    item.expires_at
        .as_deref()
        .map(|e| e < now.as_str())
        .unwrap_or(false)
}

/// Retrieve scoped, fresh items from `store` and wrap them in a `SourceManifest`.
pub fn sources(
    store: &dyn KnowledgeStore,
    query: &str,
    scope: &Scope,
    limit: u32,
    clock: &dyn Clock,
) -> Result<SourceManifest, KnowledgeError> {
    let listed = store.list(query, scope, limit)?;
    let fresh: Vec<KnowledgeItem> = listed
        .into_iter()
        .filter(|i| !check_expiry(i, clock))
        .collect();
    let n = fresh.len() as u32;
    Ok(SourceManifest { checked: n, total: n, items: fresh })
}
