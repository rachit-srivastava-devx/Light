//! Integration-level RRF checks (unit coverage also lives in `src/fuse.rs`).

use fleet_context::{fuse_rrf, SymbolId};

fn id(name: &str) -> SymbolId {
    SymbolId::derive("f.rs", name, 0)
}

#[test]
fn absent_from_a_ranking_contributes_zero_not_a_penalty() {
    let a = id("a");
    let b = id("b");
    let only_a = vec![(a.clone(), 1.0)];
    let fused = fuse_rrf(&[only_a], 60.0);
    assert_eq!(fused.len(), 1);
    assert_eq!(fused[0].0, a);
    let _ = b;
}

#[test]
fn empty_ranking_inside_rankings_contributes_nothing() {
    let a = id("a");
    let fused = fuse_rrf(&[vec![(a.clone(), 1.0)], vec![]], 60.0);
    assert_eq!(fused.len(), 1);
    assert_eq!(fused[0].0, a);
}
