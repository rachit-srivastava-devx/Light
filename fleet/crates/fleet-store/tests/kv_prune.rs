//! `KvStore::usage()`/`KvStore::prune()`: limit enforcement, no-op on empty, usage accuracy.

use fleet_store::{KvStore, RetentionPolicy};

#[test]
fn kv_prune_is_a_noop_on_an_empty_store() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = KvStore::open(&dir.path().join("kv.redb")).unwrap();
    assert_eq!(store.usage().unwrap().row_count, 0);
    let report = store.prune(&RetentionPolicy { max_rows: Some(0), ..Default::default() }).unwrap();
    assert_eq!(report.rows_removed, 0);
}

#[test]
fn kv_prune_respects_max_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = KvStore::open(&dir.path().join("kv.redb")).unwrap();
    for i in 0..5u8 {
        store.put("t", &[i], b"v").unwrap();
    }
    assert_eq!(store.usage().unwrap().row_count, 5);
    let report = store.prune(&RetentionPolicy { max_rows: Some(2), ..Default::default() }).unwrap();
    assert_eq!(report.rows_removed, 3);
    assert_eq!(store.usage().unwrap().row_count, 2);
}

#[test]
fn kv_usage_matches_what_was_actually_written() {
    let dir = tempfile::tempdir().unwrap();
    let store = KvStore::open(&dir.path().join("kv.redb")).unwrap();
    store.put("t", b"a", b"1").unwrap();
    store.put("t", b"b", b"2").unwrap();
    store.put("t", b"c", b"3").unwrap();
    let usage = store.usage().unwrap();
    assert_eq!(usage.row_count, 3);
    assert!(usage.byte_size > 0);
}
