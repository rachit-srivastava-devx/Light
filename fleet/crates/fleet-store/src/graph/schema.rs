//! `open()`: connection + `CREATE TABLE`/`INDEX`, ported from `graph.rs:612-651`'s `open_db`.

use std::fs;
use std::path::Path;

use rusqlite::Connection;

use super::types::GraphError;
use super::GraphStore;
use crate::io_fault::IoFault;

impl GraphStore {
    /// Open (creating if absent) and ensure the schema exists.
    pub fn open(path: &Path) -> Result<Self, GraphError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|source| IoFault::Open { path: path.to_path_buf(), source })?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS projects(
               project_id TEXT PRIMARY KEY, root_path TEXT NOT NULL UNIQUE, tree_digest TEXT NOT NULL,
               indexed_commit TEXT NOT NULL, indexed_file_count INTEGER NOT NULL,
               floor INTEGER NOT NULL, files_skipped INTEGER NOT NULL, languages TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS files(
               project_id TEXT NOT NULL, path TEXT NOT NULL, language TEXT NOT NULL, digest TEXT NOT NULL,
               PRIMARY KEY(project_id, path)
             );
             CREATE TABLE IF NOT EXISTS symbols(
               project_id TEXT NOT NULL, symbol_id TEXT NOT NULL, path TEXT NOT NULL, name TEXT NOT NULL,
               arity INTEGER NOT NULL, kind TEXT NOT NULL, line INTEGER NOT NULL,
               PRIMARY KEY(project_id, symbol_id)
             );
             CREATE INDEX IF NOT EXISTS symbols_name ON symbols(project_id, name);
             CREATE TABLE IF NOT EXISTS edges(
               project_id TEXT NOT NULL, caller_id TEXT NOT NULL, callee_id TEXT NOT NULL,
               PRIMARY KEY(project_id, caller_id, callee_id)
             );
             CREATE TABLE IF NOT EXISTS aliases(
               project_id TEXT NOT NULL, path TEXT NOT NULL, name TEXT NOT NULL, arity INTEGER NOT NULL,
               from_commit TEXT NOT NULL, to_commit TEXT NOT NULL, symbol_id TEXT NOT NULL,
               PRIMARY KEY(project_id, path, name, arity, from_commit, to_commit)
             );
             CREATE INDEX IF NOT EXISTS aliases_name ON aliases(project_id, name);",
        )?;
        Ok(Self { conn, db_path: path.to_path_buf() })
    }
}
