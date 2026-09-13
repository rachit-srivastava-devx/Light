use dag::{validate, DagError, GraphVersion, Node};

fn make_node(id: &str, deps: &[&str]) -> Node {
    Node {
        id: id.to_string(),
        depends_on: deps.iter().map(|s| s.to_string()).collect(),
        read_set: vec![],
        write_set: vec![],
    }
}

/// Build a DAG that contains a cycle (chain + back-edge from first to last).
fn make_cycle_dag(n: usize, seed: usize) -> GraphVersion {
    let mut nodes: Vec<Node> = (0..n).map(|i| make_node(&format!("n{i}"), &[])).collect();
    // Chain edges: n[i] depends on n[i-1]
    for (i, node) in nodes.iter_mut().enumerate().skip(1) {
        node.depends_on.push(format!("n{}", i - 1));
    }
    // Back-edge: n0 depends on n[n-1], creating a cycle
    nodes[0].depends_on.push(format!("n{}", n - 1));
    GraphVersion {
        id: format!("cyc{n}_{seed}"),
        revision: (seed as u64) + 1,
        nodes,
    }
}

#[test]
fn property_64_seeded_cycles() {
    let mut count = 0u32;
    for n in 2..=9 {
        for seed in 0..8 {
            let g = make_cycle_dag(n, seed);
            assert_eq!(
                validate(&g),
                Err(DagError::Cycle),
                "expected Cycle for n={n} seed={seed}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 64);
}
