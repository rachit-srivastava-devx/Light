//! `Blake3Hash`/`PrevHash` edge cases (BLUEPRINT.md §9), split from `receipt_roundtrip.rs` to
//! keep test files within the crate's 80-line-per-file rule.

use fleet_types::{Blake3Hash, PrevHash};

#[test]
fn blake3_hash_requires_prefix_and_exact_hex_length() {
    let good = format!("blake3:{}", "a".repeat(64));
    assert!(Blake3Hash::parse(good).is_ok());
    assert!(Blake3Hash::parse("").is_err());
    assert!(Blake3Hash::parse(format!("blake3:{}", "a".repeat(63))).is_err());
    assert!(Blake3Hash::parse(format!("blake3:{}", "A".repeat(64))).is_err());
    assert!(Blake3Hash::parse("a".repeat(64)).is_err());
}

#[test]
fn prev_hash_has_exactly_two_representable_variants() {
    assert_eq!(PrevHash::parse("GENESIS").unwrap(), PrevHash::Genesis);
    let good = format!("blake3:{}", "b".repeat(64));
    assert!(matches!(PrevHash::parse(good).unwrap(), PrevHash::Hash(_)));
    assert!(PrevHash::parse("neither").is_err());
}

#[test]
fn prev_hash_round_trips_through_json() {
    let genesis: PrevHash = serde_json::from_str("\"GENESIS\"").unwrap();
    assert_eq!(genesis, PrevHash::Genesis);
    assert_eq!(serde_json::to_string(&genesis).unwrap(), "\"GENESIS\"");
}
