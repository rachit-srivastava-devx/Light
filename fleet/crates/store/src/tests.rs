use crate::{migrate, Event, Record, SqlStore, Store, StoreError};
use rusqlite::Connection;

fn mem_store() -> SqlStore {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    SqlStore::from_conn(conn)
}

#[test]
fn migration_round_trip() {
    let conn = Connection::open_in_memory().unwrap();
    let r1 = migrate(&conn).unwrap();
    assert_eq!(r1.checked, 3, "all three migrations must be verified");
    assert_eq!(
        r1.total, 3,
        "total must equal the number of known migrations"
    );
    // Second call must be idempotent — same connection, tables already present.
    let r2 = migrate(&conn).unwrap();
    assert_eq!(r2.checked, 3);
    assert_eq!(r2.total, 3);
}

#[test]
fn cas_rejects_stale_revision() {
    let mut store = mem_store();
    let key = "k1".to_string();
    let rec = Record {
        key: key.clone(),
        revision: 0,
        payload: b"v1".to_vec(),
    };
    let rev1 = store.cas(key.clone(), 0, rec.clone()).unwrap();
    // Use the original expected=0 again — revision is now 1, so 0 is stale.
    let err = store.cas(key.clone(), 0, rec).unwrap_err();
    assert!(
        matches!(err, StoreError::StaleRevision { current } if current == rev1),
        "expected StaleRevision with current={rev1}, got {err:?}",
    );
}

#[test]
fn crash_recovery_restores_committed_events() {
    let path = std::env::temp_dir().join(format!("store_cr_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let conn = Connection::open(&path).unwrap();
        migrate(&conn).unwrap();
        let mut store = SqlStore::from_conn(conn);
        store
            .append_event(
                0,
                Event {
                    id: "e1".to_string(),
                    payload: b"hello".to_vec(),
                },
            )
            .unwrap();
    }
    // Drop the connection — simulates process restart / crash recovery.
    let conn2 = Connection::open(&path).unwrap();
    let store2 = SqlStore::from_conn(conn2);
    let rec = store2
        .load("e1".to_string())
        .unwrap()
        .expect("committed event must survive connection drop");
    assert_eq!(rec.payload, b"hello");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn incomplete_blob_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    // Empty payload must be rejected before any I/O.
    let err = crate::artifact::publish_blob(&[], "ref1", dir.path()).unwrap_err();
    assert!(matches!(err, StoreError::IncompleteBlob));
}
