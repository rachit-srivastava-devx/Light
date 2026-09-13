/// Integration tests that exercise real behaviour not covered by unit tests.
/// These live here (outside src/) so the 80-line src/ constraint does not apply.
use store::{migrate, Event, SqlStore, Store, StoreError};

fn mem_store() -> SqlStore {
    // store::Connection is rusqlite::Connection re-exported; open_in_memory is
    // an inherent fn so it resolves without importing rusqlite directly.
    let conn = store::Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    SqlStore::from_conn(conn)
}

/// Catches mutation: `new_rev = current + 1` → `current * 1`.
/// If `*` replaces `+`, new_rev stays at 0 and the revision never increments.
#[test]
fn append_event_revision_increments() {
    let mut s = mem_store();
    let c = s
        .append_event(
            0,
            Event {
                id: "e1".to_string(),
                payload: b"x".to_vec(),
            },
        )
        .unwrap();
    assert_eq!(c.revision, 1, "first event must receive revision 1");
}

/// Catches mutation: `if dup > 0` → `if dup < 0`.
/// If `<` replaces `>`, duplicate events are silently accepted.
#[test]
fn duplicate_event_is_rejected() {
    let mut s = mem_store();
    s.append_event(
        0,
        Event {
            id: "ev".to_string(),
            payload: b"a".to_vec(),
        },
    )
    .unwrap();
    // Same id, next expected revision (1) — must fail with DuplicateEvent.
    let err = s
        .append_event(
            1,
            Event {
                id: "ev".to_string(),
                payload: b"b".to_vec(),
            },
        )
        .unwrap_err();
    assert!(
        matches!(err, StoreError::DuplicateEvent { .. }),
        "duplicate event id must be rejected; got {err:?}"
    );
}

/// Catches mutation: `!=` → `==` in `if sha256_hex(payload) != ref_id`.
/// With `==`, mismatched digests pass and matched ones are rejected.
#[test]
fn blob_digest_mismatch_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let err = store::publish_blob(b"real data", "not-the-right-hash", dir.path()).unwrap_err();
    assert!(
        matches!(err, StoreError::DigestMismatch),
        "wrong ref_id must return DigestMismatch; got {err:?}"
    );
}

/// Catches mutations: sha256_hex → `String::new()` or `"xyzzy".into()`.
/// Also catches `!=` → `==` (a valid blob would be rejected).
#[test]
fn valid_blob_is_written_atomically() {
    use sha2::Digest;
    let dir = tempfile::tempdir().unwrap();
    let payload = b"deterministic content for sha256 test";
    let ref_id = format!("{:x}", sha2::Sha256::digest(payload));
    // Must succeed: correct sha256, non-empty payload, writable dir.
    store::publish_blob(payload, &ref_id, dir.path())
        .expect("valid blob must be accepted and written");
    // Verify the bytes landed at the expected path.
    let written = std::fs::read(dir.path().join(&ref_id)).unwrap();
    assert_eq!(written, payload, "blob content must be intact after write");
}
