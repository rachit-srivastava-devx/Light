//! `build_repo_map`: parse each file, assign deterministic `SymbolId`s, resolve call edges
//! (`repomap_edges::resolve_callee`, matching `graph.rs:977-1035`'s heuristic), then score with
//! `pagerank`. File parsing itself lives in `repomap_parse`.

use crate::error::ContextError;
use crate::pagerank::pagerank;
use crate::parse::RawCall;
use crate::repomap_edges::resolve_callee;
use crate::repomap_parse::parse_file;
use crate::types::{RepoMap, SourceFile, SymbolId};
use std::collections::BTreeMap;

pub fn build_repo_map(files: &[SourceFile]) -> Result<RepoMap, ContextError> {
    let mut all_refs = Vec::new();
    let mut by_key: BTreeMap<(String, u64), Vec<SymbolId>> = BTreeMap::new();
    let mut per_file_calls: Vec<(Vec<SymbolId>, Vec<RawCall>)> = Vec::new();

    for file in files {
        let (refs, ids, calls) = parse_file(file)?;
        for symbol_ref in &refs {
            by_key
                .entry((symbol_ref.name.clone(), symbol_ref.arity))
                .or_default()
                .push(symbol_ref.id.clone());
        }
        all_refs.extend(refs);
        per_file_calls.push((ids, calls));
    }

    let mut edges = std::collections::BTreeSet::new();
    for (ids, calls) in &per_file_calls {
        for call in calls {
            let Some(caller) = ids.get(call.caller) else {
                continue;
            };
            if let Some(target) = resolve_callee(&by_key, &call.callee_name, call.arity) {
                edges.insert((caller.clone(), target));
            }
        }
    }
    let edges: Vec<(SymbolId, SymbolId)> = edges.into_iter().collect();
    let node_ids: Vec<SymbolId> = all_refs.iter().map(|s| s.id.clone()).collect();
    let importance = pagerank(&node_ids, &edges, 0.85, 50);

    Ok(RepoMap {
        symbols: all_refs,
        edges,
        importance,
    })
}
