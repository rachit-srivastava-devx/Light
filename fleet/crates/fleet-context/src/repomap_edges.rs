//! Callee resolution: exact `(name, arity)` match if unique, else unique-name-only match, else
//! drop. Ported logic from `graph.rs:977-1035`'s `resolve_edges`, never guessing between two
//! ambiguous candidates.

use crate::types::SymbolId;
use std::collections::BTreeMap;

pub fn resolve_callee(
    by_key: &BTreeMap<(String, u64), Vec<SymbolId>>,
    name: &str,
    arity: u64,
) -> Option<SymbolId> {
    if let Some(ids) = by_key.get(&(name.to_string(), arity)) {
        if ids.len() == 1 {
            return ids.first().cloned();
        }
    }
    let mut same_name = by_key
        .iter()
        .filter(|((candidate_name, _), _)| candidate_name == name)
        .flat_map(|(_, ids)| ids.iter());
    let first = same_name.next();
    if first.is_some() && same_name.next().is_none() {
        first.cloned()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambiguous_callee_never_guessed() {
        let mut by_key: BTreeMap<(String, u64), Vec<SymbolId>> = BTreeMap::new();
        by_key.insert(
            ("f".to_string(), 0),
            vec![SymbolId::derive("a.rs", "f", 0), SymbolId::derive("b.rs", "f", 0)],
        );
        assert_eq!(resolve_callee(&by_key, "f", 0), None);
    }

    #[test]
    fn unique_name_arity_match_resolves() {
        let id = SymbolId::derive("a.rs", "f", 1);
        let mut by_key: BTreeMap<(String, u64), Vec<SymbolId>> = BTreeMap::new();
        by_key.insert(("f".to_string(), 1), vec![id.clone()]);
        assert_eq!(resolve_callee(&by_key, "f", 1), Some(id));
    }
}
