//! `replace_project()`: the transactional write half of `index_command`, `graph.rs:390-484`.

use rusqlite::params;

use super::batch::ReindexBatch;
use super::types::GraphError;
use super::GraphStore;

impl GraphStore {
    /// Replace one project's `files`/`symbols`/`edges`/`aliases` and upsert its `projects` row,
    /// all inside one transaction. Either every row lands or none does.
    pub fn replace_project(&mut self, batch: ReindexBatch) -> Result<(), GraphError> {
        let project_id = batch.project.project_id.clone();
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO projects(project_id, root_path, tree_digest, indexed_commit, indexed_file_count, floor, files_skipped, languages)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(root_path) DO UPDATE SET project_id=excluded.project_id, tree_digest=excluded.tree_digest,
             indexed_commit=excluded.indexed_commit, indexed_file_count=excluded.indexed_file_count,
             floor=excluded.floor, files_skipped=excluded.files_skipped, languages=excluded.languages",
            params![
                project_id,
                batch.project.root_path.to_string_lossy().as_ref(),
                batch.project.tree_digest,
                batch.project.indexed_commit,
                batch.project.indexed_file_count as i64,
                batch.project.floor as i64,
                batch.project.files_skipped as i64,
                batch.project.languages.join(","),
            ],
        )?;
        tx.execute("DELETE FROM edges WHERE project_id=?1", params![project_id])?;
        tx.execute("DELETE FROM files WHERE project_id=?1", params![project_id])?;
        tx.execute("DELETE FROM symbols WHERE project_id=?1", params![project_id])?;
        for file in &batch.files {
            tx.execute(
                "INSERT INTO files(project_id, path, language, digest) VALUES(?1, ?2, ?3, ?4)",
                params![project_id, file.path, file.language, file.digest],
            )?;
        }
        for symbol in &batch.symbols {
            tx.execute(
                "INSERT INTO symbols(project_id, symbol_id, path, name, arity, kind, line) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    project_id, symbol.symbol_id, symbol.path, symbol.name,
                    symbol.arity as i64, symbol.kind, symbol.line as i64
                ],
            )?;
        }
        for edge in &batch.edges {
            tx.execute(
                "INSERT OR IGNORE INTO edges(project_id, caller_id, callee_id) VALUES(?1, ?2, ?3)",
                params![project_id, edge.caller_id, edge.callee_id],
            )?;
        }
        for alias in &batch.aliases {
            tx.execute(
                "INSERT OR IGNORE INTO aliases(project_id, path, name, arity, from_commit, to_commit, symbol_id)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    project_id, alias.path, alias.name, alias.arity as i64,
                    alias.from_commit, alias.to_commit, alias.symbol_id
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
