use std::collections::{HashMap, HashSet, VecDeque};

use crate::types::{DagError, GraphVersion};

/// Validate a GraphVersion: nonempty, unique IDs, all deps present, acyclic.
pub fn validate(version: &GraphVersion) -> Result<(), DagError> {
    if version.nodes.is_empty() {
        return Err(DagError::Empty);
    }
    // unique IDs
    let mut seen: HashSet<&str> = HashSet::new();
    for n in &version.nodes {
        if !seen.insert(n.id.as_str()) {
            return Err(DagError::Duplicate(n.id.clone()));
        }
    }
    // all deps must name existing nodes
    for n in &version.nodes {
        for dep in &n.depends_on {
            if !seen.contains(dep.as_str()) {
                return Err(DagError::MissingDependency(dep.clone()));
            }
        }
    }
    // Kahn's algorithm: indegree = number of declared dependencies per node
    let mut indegree: HashMap<&str, usize> =
        version.nodes.iter().map(|n| (n.id.as_str(), n.depends_on.len())).collect();
    // successors: who depends on this node (edges: dep -> dependent)
    let mut successors: HashMap<&str, Vec<&str>> =
        version.nodes.iter().map(|n| (n.id.as_str(), Vec::new())).collect();
    for n in &version.nodes {
        for dep in &n.depends_on {
            successors.entry(dep.as_str()).or_default().push(n.id.as_str());
        }
    }
    let mut queue: VecDeque<&str> =
        indegree.iter().filter(|(_, &v)| v == 0).map(|(&k, _)| k).collect();
    let mut processed = 0usize;
    while let Some(id) = queue.pop_front() {
        processed += 1;
        for &suc in &successors[id] {
            let e = indegree.get_mut(suc).unwrap();
            *e -= 1;
            if *e == 0 {
                queue.push_back(suc);
            }
        }
    }
    if processed != version.nodes.len() {
        return Err(DagError::Cycle);
    }
    Ok(())
}
