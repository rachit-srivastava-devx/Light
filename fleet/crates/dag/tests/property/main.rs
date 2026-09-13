use dag::{validate, GraphVersion, Node};

fn make_node(id: &str, deps: &[&str]) -> Node {
    Node {
        id: id.to_string(),
        depends_on: deps.iter().map(|s| s.to_string()).collect(),
        read_set: vec![],
        write_set: vec![],
    }
}

/// Build a valid acyclic DAG with `n` nodes and a structure determined by `seed`.
fn make_valid_dag(n: usize, seed: usize) -> GraphVersion {
    let nodes = (0..n)
        .map(|i| {
            let dep = match seed % 3 {
                0 => {
                    // chain: each node depends on previous
                    if i > 0 {
                        vec![format!("n{}", i - 1)]
                    } else {
                        vec![]
                    }
                }
                1 => {
                    // star: all nodes depend on node 0
                    if i > 0 {
                        vec!["n0".to_string()]
                    } else {
                        vec![]
                    }
                }
                _ => {
                    // binary-tree: node i depends on node i/2
                    if i > 1 {
                        vec![format!("n{}", i / 2)]
                    } else {
                        vec![]
                    }
                }
            };
            make_node(
                &format!("n{i}"),
                &dep.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            )
        })
        .collect();
    GraphVersion {
        id: format!("g{n}_{seed}"),
        revision: (seed as u64) + 1,
        nodes,
    }
}

#[test]
fn property_256_valid_dags() {
    let mut count = 0u32;
    for n in 1..=16 {
        for seed in 0..16 {
            let g = make_valid_dag(n, seed);
            assert!(validate(&g).is_ok(), "expected Ok for n={n} seed={seed}");
            count += 1;
        }
    }
    assert_eq!(count, 256);
}
