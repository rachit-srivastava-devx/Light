use crate::StoreError;
use rusqlite::Connection;

pub struct MigrationReport {
    pub checked: u32,
    pub total: u32,
}

/// Ensure the ingest tables exist for both new and already-migrated stores.
pub fn ensure_ingest_schema(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _ingest_migrations (id INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT(datetime('now')));
         CREATE TABLE IF NOT EXISTS ingest_events (event_id TEXT PRIMARY KEY, source TEXT NOT NULL, schema_version INTEGER NOT NULL DEFAULT 1, payload_ref TEXT, delivery_id TEXT NOT NULL, object_version TEXT NOT NULL, object_version_position INTEGER, external_actor TEXT NOT NULL, auth_metadata_ref TEXT NOT NULL, payload BLOB NOT NULL, payload_digest TEXT NOT NULL, injection_taint INTEGER NOT NULL, stale_object_version INTEGER NOT NULL DEFAULT 0, UNIQUE(source,delivery_id));
         CREATE TABLE IF NOT EXISTS ingest_redactions (event_id TEXT PRIMARY KEY, categories BLOB NOT NULL, field_count INTEGER NOT NULL, FOREIGN KEY(event_id) REFERENCES ingest_events(event_id));
         CREATE TABLE IF NOT EXISTS ingest_attachments (event_id TEXT NOT NULL, uri TEXT NOT NULL, digest TEXT NOT NULL, mime_type TEXT NOT NULL, size_bytes INTEGER NOT NULL, PRIMARY KEY(event_id,uri), FOREIGN KEY(event_id) REFERENCES ingest_events(event_id));
         CREATE TABLE IF NOT EXISTS ingest_cursors (source TEXT PRIMARY KEY, cursor TEXT NOT NULL, cursor_position INTEGER NOT NULL DEFAULT 0, revision INTEGER NOT NULL);
         CREATE TABLE IF NOT EXISTS ingest_refusals (id INTEGER PRIMARY KEY AUTOINCREMENT, source TEXT NOT NULL, delivery_id TEXT NOT NULL, error TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT(datetime('now')));",
    )
    .map_err(|e| StoreError::Migration(e.to_string()))?;
    add_ingest_column(conn, "object_version", "TEXT NOT NULL DEFAULT ''")?;
    add_ingest_column(conn, "external_actor", "TEXT NOT NULL DEFAULT ''")?;
    add_ingest_column(conn, "auth_metadata_ref", "TEXT NOT NULL DEFAULT ''")?;
    add_ingest_column(conn, "schema_version", "INTEGER NOT NULL DEFAULT 1")?;
    add_ingest_column(conn, "payload_ref", "TEXT")?;
    add_ingest_column(conn, "object_version_position", "INTEGER")?;
    add_ingest_column(conn, "stale_object_version", "INTEGER NOT NULL DEFAULT 0")?;
    add_cursor_column(conn, "cursor_position", "INTEGER NOT NULL DEFAULT 0")?;
    conn.execute("INSERT OR IGNORE INTO _ingest_migrations(id) VALUES(1)", [])
        .map_err(|e| StoreError::Migration(e.to_string()))?;
    Ok(())
}

fn add_cursor_column(conn: &Connection, name: &str, definition: &str) -> Result<(), StoreError> {
    let mut statement = conn
        .prepare("PRAGMA table_info(ingest_cursors)")
        .map_err(|e| StoreError::Migration(e.to_string()))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| StoreError::Migration(e.to_string()))?;
    let mut exists = false;
    for column in columns {
        if column.map_err(|e| StoreError::Migration(e.to_string()))? == name {
            exists = true;
            break;
        }
    }
    if !exists {
        conn.execute(
            &format!("ALTER TABLE ingest_cursors ADD COLUMN {name} {definition}"),
            [],
        )
        .map_err(|e| StoreError::Migration(e.to_string()))?;
    }
    Ok(())
}

fn add_ingest_column(conn: &Connection, name: &str, definition: &str) -> Result<(), StoreError> {
    let mut statement = conn
        .prepare("PRAGMA table_info(ingest_events)")
        .map_err(|e| StoreError::Migration(e.to_string()))?;
    let mut exists = false;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| StoreError::Migration(e.to_string()))?;
    for column in columns {
        if column.map_err(|e| StoreError::Migration(e.to_string()))? == name {
            exists = true;
            break;
        }
    }
    if !exists {
        conn.execute(
            &format!("ALTER TABLE ingest_events ADD COLUMN {name} {definition}"),
            [],
        )
        .map_err(|e| StoreError::Migration(e.to_string()))?;
    }
    Ok(())
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
            .query_row("SELECT COUNT(*) FROM _migrations WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .map_err(|e| StoreError::Migration(e.to_string()))?;
        if applied == 0 {
            conn.execute_batch(sql)
                .map_err(|e| StoreError::Migration(e.to_string()))?;
            conn.execute("INSERT OR IGNORE INTO _migrations(id) VALUES(?1)", [id])
                .map_err(|e| StoreError::Migration(e.to_string()))?;
        }
        checked += 1; // confirmed present (just applied or previously applied)
    }
    ensure_ingest_schema(conn)?;
    Ok(MigrationReport { checked, total })
}
