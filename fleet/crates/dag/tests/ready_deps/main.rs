use dag::{ready, validate, GraphVersion, Node};

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
fn accepted_dependency_gates_ready() {
    // A (root) → B → C; accepted=["A"]; only B satisfies its deps
    let v = ver(vec![node("A", &[]), node("B", &["A"]), node("C", &["B"])]);
    let rs = ready(&v, &["A".to_string()]).unwrap();
    assert_eq!(rs.ids, vec!["B".to_string()]);
}

#[test]
fn empty_accepted_set_means_only_roots_ready() {
    // A (root), B depends on A; nothing accepted → only root A is ready
    let v = ver(vec![node("A", &[]), node("B", &["A"])]);
    let rs = ready(&v, &[]).unwrap();
    assert_eq!(rs.ids, vec!["A".to_string()]);
}

#[test]
fn validate_called_before_ready_on_valid_graph() {
    // Explicit validate then ready both succeed and agree on ready set
    let v = ver(vec![node("X", &[]), node("Y", &["X"])]);
    validate(&v).unwrap();
    let rs = ready(&v, &[]).unwrap();
    assert_eq!(rs.ids, vec!["X".to_string()]);
}
