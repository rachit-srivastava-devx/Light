//! `upsert`/`bm25_search`/`vector_search`, dimension mismatch.
//!
//! `MemoryStore::open` loads the `vec0` sqlite extension in-process (BLUEPRINT.md §5's
//! replacement for the Python subprocess bridge). No `vec0.{dylib,so,dll}` artifact is installed
//! in this build/test environment (no `sqlite-vec` Python package, no vendored copy found in the
//! repo) -- per the build brief, vector-dependent assertions are gated behind its availability via
//! `FLEET_STORE_TEST_VEC0` and skipped (not failed) when it's unset/missing.

use fleet_store::MemoryStore;

mod support;
use support::{mem_row, vec0_path};

#[test]
fn memory_upsert_increments_confirmed_count_and_search_returns_unfused_lists() {
    let Some(vec0) = vec0_path() else {
        eprintln!("skipping memory_search: FLEET_STORE_TEST_VEC0 not set to an existing vec0 extension");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(&dir.path().join("mem.sqlite3"), 4, &vec0).unwrap();

    let rowid = store.upsert(mem_row("m1", "t1", "sqlite tuning tips")).unwrap();
    let rowid_again = store.upsert(mem_row("m1", "t1-updated", "sqlite tuning tips v2")).unwrap();
    assert_eq!(rowid, rowid_again);

    store.set_embedding(rowid, &[1.0, 0.0, 0.0, 0.0], "hash-1").unwrap();
    let hashes = store.embedding_hashes().unwrap();
    assert_eq!(hashes, vec![(rowid, "hash-1".to_string())]);

    let bm25 = store.bm25_search(&["sqlite".to_string()], 10).unwrap();
    assert_eq!(bm25.len(), 1);
    assert_eq!(bm25[0].id, "m1");

    let vector = store.vector_search(&[1.0, 0.0, 0.0, 0.0], 10).unwrap();
    assert_eq!(vector.len(), 1);
    assert_eq!(vector[0].id, "m1");
}

#[test]
fn memory_set_embedding_rejects_dimension_mismatch() {
    let Some(vec0) = vec0_path() else {
        eprintln!("skipping memory_search: FLEET_STORE_TEST_VEC0 not set to an existing vec0 extension");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(&dir.path().join("mem.sqlite3"), 384, &vec0).unwrap();
    let rowid = store.upsert(mem_row("m1", "t1", "body")).unwrap();
    let err = store.set_embedding(rowid, &[0.0; 128], "hash").unwrap_err();
    assert!(matches!(
        err,
        fleet_store::memory::MemoryError::DimensionMismatch { expected: 384, found: 128 }
    ));
}

#[test]
fn bm25_search_with_no_terms_returns_empty_without_matching_all() {
    let Some(vec0) = vec0_path() else {
        eprintln!("skipping memory_search: FLEET_STORE_TEST_VEC0 not set to an existing vec0 extension");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(&dir.path().join("mem.sqlite3"), 4, &vec0).unwrap();
    store.upsert(mem_row("m1", "t1", "body")).unwrap();
    assert_eq!(store.bm25_search(&[], 10).unwrap(), vec![]);
}
