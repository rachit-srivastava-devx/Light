//! `dag` — versioned workflow DAG: cycle detection, ready ordering, CAS port.
//! Blueprint: docs/blueprints-next/dag/BLUEPRINT.md

mod graph;
pub mod port;
mod ready;
pub mod types;

pub use graph::validate;
pub use port::{MemoryVersionStore, VersionStore};
pub use ready::ready;
pub use types::{DagError, GraphVersion, Node, ReadySet, SkipPlanningSignal, WorkflowRecipe};

#[cfg(test)]
mod tests {
    use super::*;
    use types::Node;

    fn mk_node(id: &str, deps: &[&str]) -> Node {
        Node {
            id: id.into(),
            depends_on: deps.iter().map(|s| s.to_string()).collect(),
            read_set: vec![],
            write_set: vec![],
        }
    }
    fn mk_gv(revision: u64, nodes: Vec<Node>) -> GraphVersion {
        GraphVersion {
            id: "g".into(),
            revision,
            nodes,
        }
    }

    #[test]
    fn cycle_detection_refuses_input() {
        let v = mk_gv(1, vec![mk_node("A", &["B"]), mk_node("B", &["A"])]);
        assert_eq!(validate(&v), Err(DagError::Cycle));
    }

    #[test]
    fn ready_set_respects_accepted_deps() {
        let v = mk_gv(1, vec![mk_node("A", &[]), mk_node("B", &["A"])]);
        let rs = ready(&v, &[]).unwrap();
        assert_eq!(rs.ids, vec!["A"]);
    }

    #[test]
    fn zero_node_graph_is_refused() {
        let v = mk_gv(1, vec![]);
        assert_eq!(validate(&v), Err(DagError::Empty));
    }

    #[test]
    fn stale_revision_refused() {
        let mut store = MemoryVersionStore::default();
        let v1 = mk_gv(1, vec![mk_node("A", &[])]);
        store.check_revision(&v1).unwrap();
        store.store(&v1).unwrap();
        let v0 = mk_gv(0, vec![mk_node("A", &[])]);
        assert_eq!(store.check_revision(&v0), Err(DagError::StaleRevision));
    }
}
