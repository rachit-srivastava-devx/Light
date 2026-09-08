//! `score()` edge cases. See BLUEPRINT.md §9.

use fleet_memory::{score, Importance, MemoryId, MemoryItem, MemoryKind, Relevance, Score, ScoreWeights, Timestamp};

fn item(last_confirmed: u64) -> MemoryItem {
    MemoryItem {
        id: MemoryId::parse("m1").unwrap(),
        kind: MemoryKind::Semantic,
        text: "x".into(),
        embedding: fleet_memory::Embedding::new(vec![1.0]).unwrap(),
        importance: Importance::new(0.0).unwrap(),
        created_at: Timestamp::from_unix_secs(0),
        last_confirmed_at: Timestamp::from_unix_secs(last_confirmed),
        confirmed_count: 0,
    }
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn recency_decays_by_half_at_one_half_life() {
    let weights = ScoreWeights { alpha: 1.0, beta: 0.0, gamma: 0.0 };
    let half_life = fleet_memory::RECENCY_HALF_LIFE_SECS;
    let now = Timestamp::from_unix_secs(half_life);
    let it = item(0);
    let s: Score = score(&it, now, Relevance::new(0.0).unwrap(), weights);
    assert!(approx(s.get(), 0.5), "expected 0.5, got {}", s.get());

    let now_zero = Timestamp::from_unix_secs(0);
    let s2 = score(&it, now_zero, Relevance::new(0.0).unwrap(), weights);
    assert!(approx(s2.get(), 1.0), "expected 1.0, got {}", s2.get());
}

#[test]
fn recency_clamps_future_last_confirmed_to_one() {
    let weights = ScoreWeights { alpha: 1.0, beta: 0.0, gamma: 0.0 };
    let it = item(1_000_000);
    let now = Timestamp::from_unix_secs(0);
    let s = score(&it, now, Relevance::new(0.0).unwrap(), weights);
    assert!(approx(s.get(), 1.0), "expected clamped recency 1.0, got {}", s.get());
}

#[test]
fn embedding_cosine_rejects_dimension_mismatch() {
    let a = fleet_memory::Embedding::new(vec![1.0, 2.0, 3.0]).unwrap();
    let b = fleet_memory::Embedding::new(vec![1.0, 2.0, 3.0, 4.0]).unwrap();
    let err = a.cosine(&b).unwrap_err();
    assert_eq!(err.a, 3);
    assert_eq!(err.b, 4);
}

#[test]
fn embedding_cosine_is_clamped_to_valid_range() {
    let a = fleet_memory::Embedding::new(vec![1.0, 1.0000001, 1.0]).unwrap();
    let b = fleet_memory::Embedding::new(vec![1.0, 1.0, 1.0]).unwrap();
    let sim = a.cosine(&b).unwrap().get();
    assert!((-1.0..=1.0).contains(&sim));
}
