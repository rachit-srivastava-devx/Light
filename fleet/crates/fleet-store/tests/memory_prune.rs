//! `MemoryStore::usage()`/`MemoryStore::prune()`: max_age/max_rows enforcement, no-op on empty.
//! Gated behind `FLEET_STORE_TEST_VEC0` like `memory_search.rs` -- `vec0` is not vendored here.

use std::time::{Duration, SystemTime};

use fleet_store::{MemoryStore, RetentionPolicy};

mod support;
use support::{mem_row, vec0_path};

#[test]
fn memory_prune_is_a_noop_on_an_empty_store() {
    let Some(vec0) = vec0_path() else {
        eprintln!("skipping memory_prune_is_a_noop_on_an_empty_store: FLEET_STORE_TEST_VEC0 unset");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(&dir.path().join("mem.sqlite3"), 4, &vec0).unwrap();
    let report =
        store.prune(&RetentionPolicy { max_rows: Some(0), ..Default::default() }, SystemTime::now()).unwrap();
    assert_eq!(report.rows_removed, 0);
}

#[test]
fn memory_prune_respects_max_rows() {
    let Some(vec0) = vec0_path() else {
        eprintln!("skipping memory_prune_respects_max_rows: FLEET_STORE_TEST_VEC0 unset");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(&dir.path().join("mem.sqlite3"), 4, &vec0).unwrap();
    for i in 0..4 {
        store.upsert(mem_row(&format!("m{i}"), "t", "body")).unwrap();
    }
    assert_eq!(store.usage().unwrap().row_count, 4);
    let report =
        store.prune(&RetentionPolicy { max_rows: Some(1), ..Default::default() }, SystemTime::now()).unwrap();
    assert_eq!(report.rows_removed, 3);
    assert_eq!(store.usage().unwrap().row_count, 1);
}

#[test]
fn memory_prune_respects_max_age() {
    let Some(vec0) = vec0_path() else {
        eprintln!("skipping memory_prune_respects_max_age: FLEET_STORE_TEST_VEC0 unset");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(&dir.path().join("mem.sqlite3"), 4, &vec0).unwrap();
    store.upsert(mem_row("m1", "t", "body")).unwrap();
    // Everything currently in the store is "older" than a cutoff far in the future.
    let far_future = SystemTime::now() + Duration::from_secs(3600);
    let report =
        store.prune(&RetentionPolicy { max_age_secs: Some(1), ..Default::default() }, far_future).unwrap();
    assert_eq!(report.rows_removed, 1);
    assert_eq!(store.usage().unwrap().row_count, 0);
}
