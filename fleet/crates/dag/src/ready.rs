use std::collections::HashSet;

use crate::types::{DagError, GraphVersion, ReadySet};

/// Return the set of nodes whose declared dependencies are all in `accepted`.
/// Nodes already in `accepted` are excluded from the result.
/// IDs are sorted lexicographically for stable ordering.
pub fn ready(version: &GraphVersion, accepted: &[String]) -> Result<ReadySet, DagError> {
    if version.nodes.is_empty() {
        return Err(DagError::Empty);
    }
    let accepted_set: HashSet<&str> = accepted.iter().map(|s| s.as_str()).collect();
    let total = version.nodes.len() as u64;
    let mut ids: Vec<&str> = version
        .nodes
        .iter()
        .filter(|n| !accepted_set.contains(n.id.as_str()))
        .filter(|n| n.depends_on.iter().all(|dep| accepted_set.contains(dep.as_str())))
        .map(|n| n.id.as_str())
        .collect();
    // stable lexicographic tie-break (apply_tie_break)
    ids.sort_unstable();
    Ok(ReadySet {
        revision: version.revision,
        ids: ids.iter().map(|s| s.to_string()).collect(),
        checked: total,
        total,
    })
}
