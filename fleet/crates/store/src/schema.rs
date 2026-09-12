use rusqlite::Connection;
use crate::StoreError;

pub struct MigrationReport {
    pub checked: u32,
    pub total: u32,
}

/// Three migration batches; each may contain multiple semicolon-delimited DDL statements.
const MIGRATIONS: &[&str] = &[
    // 1: core event log and node state
    "CREATE TABLE IF NOT EXISTS events \
     (id TEXT PRIMARY KEY, revision INTEGER NOT NULL, payload BLOB NOT NULL); \
     CREATE TABLE IF NOT EXISTS nodes \
     (key TEXT PRIMARY KEY, revision INTEGER NOT NULL, payload BLOB NOT NULL)",
    // 2: messaging and receipts
    "CREATE TABLE IF NOT EXISTS inbox \
     (id TEXT PRIMARY KEY, source TEXT NOT NULL, payload BLOB NOT NULL); \
     CREATE TABLE IF NOT EXISTS outbox \
     (id TEXT PRIMARY KEY, idempotency_key TEXT UNIQUE NOT NULL, payload BLOB NOT NULL); \
     CREATE TABLE IF NOT EXISTS receipts \
     (id TEXT PRIMARY KEY, event_id TEXT NOT NULL, \
      created_at TEXT NOT NULL DEFAULT (datetime('now')))",
    // 3: leases and blobs
    "CREATE TABLE IF NOT EXISTS leases \
     (id TEXT PRIMARY KEY, generation INTEGER NOT NULL, holder TEXT NOT NULL); \
     CREATE TABLE IF NOT EXISTS blobs \
     (id TEXT PRIMARY KEY, sha256 TEXT NOT NULL, size INTEGER NOT NULL, path TEXT NOT NULL)",
];

fn bootstrap(conn: &Connection) -> Result<(), StoreError> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='_migrations'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| StoreError::Migration(e.to_string()))?;
    if n == 0 {
        conn.execute_batch(
            "CREATE TABLE _migrations \
             (id INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (datetime('now')))",
        )
        .map_err(|e| StoreError::Migration(e.to_string()))?;
    }
    Ok(())
}

/// Apply all pending migrations; idempotent on repeated calls.
/// `checked` counts migrations verified (applied now or confirmed already applied).
/// `total` is the number of known migrations.
pub fn migrate(conn: &Connection) -> Result<MigrationReport, StoreError> {
    bootstrap(conn)?;
    let total = MIGRATIONS.len() as u32;
    let mut checked = 0u32;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let id = i as i64 + 1;
        let applied: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM _migrations WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(|e| StoreError::Migration(e.to_string()))?;
        if applied == 0 {
            conn.execute_batch(sql)
                .map_err(|e| StoreError::Migration(e.to_string()))?;
            conn.execute("INSERT OR IGNORE INTO _migrations(id) VALUES(?1)", [id])
                .map_err(|e| StoreError::Migration(e.to_string()))?;
        }
        checked += 1; // confirmed present (just applied or previously applied)
    }
    Ok(MigrationReport { checked, total })
}
