//! `write()` Insert/Merge boundary cases + dedup-reduces-inserts property. See BLUEPRINT.md §9.

use fleet_memory::{
    write, CosineSimilarity, DedupThreshold, Embedding, Importance, MemoryId, MemoryKind,
    NearestNeighborLookup, NewMemory, RetrieveError, Timestamp, WriteDecision,
};

struct FixedNeighbor(Option<(MemoryId, CosineSimilarity)>);
impl NearestNeighborLookup for FixedNeighbor {
    fn nearest(&self, _e: &Embedding) -> Result<Option<(MemoryId, CosineSimilarity)>, RetrieveError> {
        Ok(self.0.clone())
    }
}

/// Two embeddings' actual cosine similarity, so a test can set `tau` to the exact same `f64`
/// bit-value the mocked port returns — a robust boundary test regardless of the value's digits.
fn near_similarity() -> CosineSimilarity {
    let a = Embedding::new(vec![1.0, 0.0]).unwrap();
    let b = Embedding::new(vec![0.9, 0.435_889_9]).unwrap();
    a.cosine(&b).unwrap()
}

fn candidate() -> NewMemory {
    NewMemory { kind: MemoryKind::Semantic, text: "x".into(), embedding: Embedding::new(vec![1.0]).unwrap(), importance: Importance::new(0.5).unwrap() }
}

#[test]
fn write_merges_at_exact_threshold_boundary() {
    let sim = near_similarity();
    let tau = DedupThreshold::new(sim.get()).unwrap();
    let neighbor = FixedNeighbor(Some((MemoryId::parse("existing").unwrap(), sim)));
    let decision = write(MemoryId::parse("new").unwrap(), candidate(), Timestamp::from_unix_secs(0), tau, &neighbor).unwrap();
    match decision {
        WriteDecision::Merge { into, .. } => assert_eq!(into.as_str(), "existing"),
        WriteDecision::Insert(_) => panic!("expected Merge at sim == tau"),
    }
}

#[test]
fn write_inserts_when_no_neighbor_exists() {
    let tau = DedupThreshold::new(0.9).unwrap();
    let neighbor = FixedNeighbor(None);
    let decision = write(MemoryId::parse("new").unwrap(), candidate(), Timestamp::from_unix_secs(0), tau, &neighbor).unwrap();
    assert!(matches!(decision, WriteDecision::Insert(_)));
}

#[test]
fn write_dedup_reduces_insert_rate_on_a_near_duplicate_stream() {
    let dup_sim = near_similarity();
    let tau = DedupThreshold::new(dup_sim.get()).unwrap();
    let mut insert_count = 0;
    let total = 9;
    for i in 0..total {
        let neighbor = if i % 3 == 2 {
            FixedNeighbor(Some((MemoryId::parse("dup-source").unwrap(), dup_sim)))
        } else {
            FixedNeighbor(None)
        };
        let decision = write(MemoryId::parse(format!("id-{i}")).unwrap(), candidate(), Timestamp::from_unix_secs(0), tau, &neighbor).unwrap();
        match decision {
            WriteDecision::Insert(_) => insert_count += 1,
            WriteDecision::Merge { into, .. } => assert_eq!(into.as_str(), "dup-source"),
        }
    }
    assert!(insert_count < total, "dedup must reduce insert count below candidate count");
}
