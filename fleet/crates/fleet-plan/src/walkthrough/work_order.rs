//! Deterministic Kahn's-algorithm topological sort of a plan's modules by `deps`, so the
//! walkthrough can say "build A, then B, then C" -- and refuse (not guess) on a cycle.

use super::extract::ModuleSummary;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct WorkOrderStep {
    pub node_id: String,
    pub because: String,
}

/// `Err` carries every node_id that never reached indegree 0 -- the modules involved in (or
/// downstream of) the cycle, reported in a stable sorted order.
pub(crate) fn compute_work_order(modules: &[ModuleSummary]) -> Result<Vec<WorkOrderStep>, Vec<String>> {
    let ids: BTreeSet<&str> = modules.iter().map(|m| m.node_id.as_str()).collect();
    let mut internal_deps: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut reverse: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut indeg: BTreeMap<&str, usize> = BTreeMap::new();

    for m in modules {
        let deps: Vec<&str> = m.deps.iter().map(String::as_str).filter(|d| ids.contains(d)).collect();
        indeg.insert(m.node_id.as_str(), deps.len());
        for &d in &deps {
            reverse.entry(d).or_default().push(m.node_id.as_str());
        }
        internal_deps.insert(m.node_id.as_str(), deps);
    }

    let mut ready: BTreeSet<&str> = indeg.iter().filter(|(_, &d)| d == 0).map(|(&k, _)| k).collect();
    let mut order: Vec<&str> = Vec::with_capacity(modules.len());
    while let Some(&next) = ready.iter().next() {
        ready.remove(next);
        order.push(next);
        for &dependent in reverse.get(next).unwrap_or(&Vec::new()) {
            let e = indeg.get_mut(dependent).expect("dependent was inserted above");
            *e -= 1;
            if *e == 0 {
                ready.insert(dependent);
            }
        }
    }

    if order.len() != modules.len() {
        let placed: BTreeSet<&str> = order.iter().copied().collect();
        let cycle: Vec<String> = ids.iter().filter(|id| !placed.contains(*id)).map(|s| s.to_string()).collect();
        return Err(cycle);
    }

    Ok(order
        .into_iter()
        .map(|node_id| {
            let deps = &internal_deps[node_id];
            let because = if deps.is_empty() {
                "no dependencies among this plan's modules".to_string()
            } else {
                format!("after: {}", deps.join(", "))
            };
            WorkOrderStep { node_id: node_id.to_string(), because }
        })
        .collect())
}
