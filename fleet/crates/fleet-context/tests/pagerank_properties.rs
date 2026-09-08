//! §9 PageRank properties: mass-conservation, dangling-node inclusion, determinism.

use fleet_context::{pagerank, SymbolId};

fn id(name: &str) -> SymbolId {
    SymbolId::derive("f.rs", name, 0)
}

#[test]
fn empty_nodes_yield_empty_map() {
    assert!(pagerank(&[], &[], 0.85, 20).is_empty());
}

#[test]
fn mass_is_conserved() {
    let a = id("a");
    let b = id("b");
    let nodes = vec![a.clone(), b.clone()];
    let edges = vec![(a.clone(), b.clone()), (b.clone(), a.clone())];
    for iterations in [20u32, 50, 200] {
        let scores = pagerank(&nodes, &edges, 0.85, iterations);
        let sum: f64 = scores.values().sum();
        assert!((sum - nodes.len() as f64).abs() < 1e-6, "sum={sum}");
    }
}

#[test]
fn deterministic_across_runs() {
    let a = id("a");
    let b = id("b");
    let nodes = vec![a.clone(), b.clone()];
    let edges = vec![(a, b)];
    let first = pagerank(&nodes, &edges, 0.85, 30);
    for _ in 0..10 {
        assert_eq!(pagerank(&nodes, &edges, 0.85, 30), first);
    }
}

#[test]
fn dangling_and_zero_indegree_nodes_are_never_omitted() {
    let a = id("a");
    let b = id("b");
    let c = id("c"); // isolated: no in-edges, no out-edges
    let nodes = vec![a.clone(), b.clone(), c.clone()];
    let edges = vec![(a, b)];
    let scores = pagerank(&nodes, &edges, 0.85, 30);
    assert_eq!(scores.len(), 3);
    assert!(scores.contains_key(&c));
}

#[test]
fn duplicate_edges_do_not_inflate_score() {
    let a = id("a");
    let b = id("b");
    let nodes = vec![a.clone(), b.clone()];
    let once = pagerank(&nodes, &[(a.clone(), b.clone())], 0.85, 30);
    let twice = pagerank(&nodes, &[(a.clone(), b.clone()), (a, b.clone())], 0.85, 30);
    assert!((once[&b] - twice[&b]).abs() < 1e-9);
}
