//! `put`/`get`/`delete`/`list_prefix`, absent-key semantics, per-table isolation.

use fleet_store::KvStore;

#[test]
fn kv_get_on_absent_key_is_ok_none_not_error() {
    let dir = tempfile::tempdir().unwrap();
    let store = KvStore::open(&dir.path().join("kv.redb")).unwrap();
    assert_eq!(store.get("t", b"missing").unwrap(), None);
}

#[test]
fn put_get_delete_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = KvStore::open(&dir.path().join("kv.redb")).unwrap();
    store.put("t", b"k1", b"v1").unwrap();
    assert_eq!(store.get("t", b"k1").unwrap(), Some(b"v1".to_vec()));
    assert!(store.delete("t", b"k1").unwrap());
    assert_eq!(store.get("t", b"k1").unwrap(), None);
    assert!(!store.delete("t", b"k1").unwrap(), "deleting an absent key returns false, not an error");
}

#[test]
fn list_prefix_returns_only_matching_keys_across_tables() {
    let dir = tempfile::tempdir().unwrap();
    let store = KvStore::open(&dir.path().join("kv.redb")).unwrap();
    store.put("table-a", b"pre:1", b"a1").unwrap();
    store.put("table-a", b"pre:2", b"a2").unwrap();
    store.put("table-a", b"other", b"a3").unwrap();
    store.put("table-b", b"pre:1", b"b1").unwrap();

    let mut hits = store.list_prefix("table-a", b"pre:").unwrap();
    hits.sort();
    assert_eq!(hits, vec![(b"pre:1".to_vec(), b"a1".to_vec()), (b"pre:2".to_vec(), b"a2".to_vec())]);
}
