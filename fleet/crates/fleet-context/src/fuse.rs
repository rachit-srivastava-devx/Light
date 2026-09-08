//! Hand-rolled Reciprocal Rank Fusion (NOT the yanked `rank-fusion` crate -- §5/§7 of the
//! blueprint). `score(id) = sum over rankings containing id of 1 / (k + rank)`, 1-based rank.

use crate::types::SymbolId;
use std::collections::BTreeMap;

pub fn fuse_rrf(rankings: &[Vec<(SymbolId, f32)>], k: f64) -> Vec<(SymbolId, f64)> {
    // Per ranking: dedupe with last-write-wins (a repeated id in one ranking should not
    // double-count, §6), then sum each ranking's contribution into the running total.
    let mut totals: BTreeMap<SymbolId, f64> = BTreeMap::new();
    for ranking in rankings {
        let mut per_ranking: BTreeMap<SymbolId, f64> = BTreeMap::new();
        for (rank, (id, _)) in ranking.iter().enumerate() {
            per_ranking.insert(id.clone(), 1.0 / (k + (rank + 1) as f64));
        }
        for (id, contribution) in per_ranking {
            *totals.entry(id).or_insert(0.0) += contribution;
        }
    }
    let mut result: Vec<(SymbolId, f64)> = totals.into_iter().collect();
    result.sort_by(|(a_id, a_score), (b_id, b_score)| {
        b_score
            .partial_cmp(a_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a_id.cmp(b_id))
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: &str) -> SymbolId {
        SymbolId::derive("f.rs", n, 0)
    }

    #[test]
    fn favors_items_ranked_highly_in_multiple_lists() {
        let a = id("a");
        let b = id("b");
        let list1 = vec![(a.clone(), 1.0), (b.clone(), 0.5)];
        let list2 = vec![(a.clone(), 1.0)];
        let fused = fuse_rrf(&[list1, list2], 60.0);
        let a_score = fused.iter().find(|(x, _)| *x == a).unwrap().1;
        let b_score = fused.iter().find(|(x, _)| *x == b).unwrap().1;
        assert!(a_score > b_score);
    }

    #[test]
    fn ties_break_by_symbol_id() {
        let a = id("z");
        let b = id("a");
        let expected_first = a.clone().min(b.clone());
        // Each ranked #1 in its own single-item ranking -> identical contribution.
        let fused = fuse_rrf(&[vec![(a, 1.0)], vec![(b, 1.0)]], 60.0);
        assert_eq!(fused[0].1, fused[1].1);
        assert_eq!(fused[0].0, expected_first);
    }

    #[test]
    fn empty_rankings_yield_empty() {
        assert!(fuse_rrf(&[], 60.0).is_empty());
    }
}
