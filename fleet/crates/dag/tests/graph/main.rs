use dag::{ready, validate, DagError, GraphVersion, Node, VersionStore};

fn node(id: &str, deps: &[&str]) -> Node {
    Node {
        id: id.to_string(),
        depends_on: deps.iter().map(|s| s.to_string()).collect(),
        read_set: vec![],
        write_set: vec![],
    }
}

fn ver(nodes: Vec<Node>) -> GraphVersion {
    GraphVersion {
        id: "test".to_string(),
        revision: 1,
        nodes,
    }
}

#[test]
fn cycle_detection_refuses_input() {
    let v = ver(vec![node("A", &["B"]), node("B", &["A"])]);
    assert_eq!(validate(&v), Err(DagError::Cycle));
}

#[test]
fn ready_set_respects_accepted_deps() {
    let v = ver(vec![node("A", &[]), node("B", &["A"])]);
    let rs = ready(&v, &[]).unwrap();
    assert_eq!(rs.ids, vec!["A".to_string()]);
}

#[test]
fn zero_node_graph_is_refused() {
    let v = GraphVersion {
        id: "e".to_string(),
        revision: 1,
        nodes: vec![],
    };
    assert_eq!(validate(&v), Err(DagError::Empty));
}

#[test]
fn stale_revision_refused() {
    let mut store = dag::MemoryVersionStore::default();
    let v1 = ver(vec![node("A", &[])]);
    store.check_revision(&v1).unwrap();
    store.store(&v1).unwrap();
    let v0 = GraphVersion {
        id: "test".to_string(),
        revision: 0,
        nodes: vec![node("A", &[])],
    };
    assert_eq!(store.check_revision(&v0), Err(DagError::StaleRevision));
}
