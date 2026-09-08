//! Deterministic text -> `fleet_memory::Embedding`, plus a stable `MemoryId` derivation. Pure:
//! no clock, no RNG, no IO -- same text always yields the same vector/id. This is a feature-
//! hashing bag-of-words embedding (FNV-1a into a fixed number of buckets), not a learned model;
//! it is honest about that trade-off (see `sow_probes.rs`) rather than pretending to be more.

use fleet_memory::{Embedding, MemoryId};

const DIMS: usize = 64;

/// FNV-1a, pure and total over any byte string.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Term-frequency-into-hashed-buckets embedding. Never returns an empty vector (`DIMS` is a
/// fixed nonzero constant), so the `Embedding::new` call below can never fail.
pub fn embed_text(text: &str) -> Embedding {
    let mut buckets = vec![0f32; DIMS];
    for word in text.split_whitespace() {
        let idx = (fnv1a(word.to_lowercase().as_bytes()) % DIMS as u64) as usize;
        buckets[idx] += 1.0;
    }
    Embedding::new(buckets).expect("DIMS is a fixed nonzero constant")
}

/// A stable id for a piece of text -- same text always maps to the same id, so a repeated
/// observation of the same requirement text is recognised as the same memory row's candidate.
pub fn stable_id(text: &str) -> MemoryId {
    MemoryId::parse(format!("sow-{:016x}", fnv1a(text.as_bytes()))).expect("hex string is non-empty")
}
