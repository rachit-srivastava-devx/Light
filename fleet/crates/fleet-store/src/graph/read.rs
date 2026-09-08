//! `project()`, `symbols()`, `symbol_ids_for()` -- ported from `graph.rs:653-694`, `:1173-1200`.

use std::collections::HashSet;
use std::path::Path;

use rusqlite::{params, OptionalExtension};

use super::types::{GraphError, ProjectRecord, SymbolRecord};
use super::GraphStore;

impl GraphStore {
    /// The project row for this root path, if one was ever indexed.
    pub fn project(&self, root_path: &Path) -> Result<Option<ProjectRecord>, GraphError> {
        self.conn
            .query_row(
                "SELECT project_id, root_path, tree_digest, indexed_commit, indexed_file_count, floor, files_skipped, languages
                 FROM projects WHERE root_path=?1",
                params![root_path.to_string_lossy().as_ref()],
                |row| {
                    let languages: String = row.get(7)?;
                    Ok(ProjectRecord {
                        project_id: row.get(0)?,
                        root_path: row.get::<_, String>(1)?.into(),
                        tree_digest: row.get(2)?,
                        indexed_commit: row.get(3)?,
                        indexed_file_count: row.get::<_, i64>(4)? as u64,
                        floor: row.get::<_, i64>(5)? as u64,
                        files_skipped: row.get::<_, i64>(6)? as u64,
                        languages: if languages.is_empty() {
                            Vec::new()
                        } else {
                            languages.split(',').map(str::to_string).collect()
                        },
                    })
                },
            )
            .optional()
            .map_err(GraphError::from)
    }

    /// Every symbol currently stored for a project.
    pub fn symbols(&self, project_id: &str) -> Result<Vec<SymbolRecord>, GraphError> {
        let mut stmt = self.conn.prepare(
            "SELECT symbol_id, path, name, arity, kind, line FROM symbols WHERE project_id=?1",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            Ok(SymbolRecord {
                symbol_id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                arity: row.get::<_, i64>(3)? as u64,
                kind: row.get(4)?,
                line: row.get::<_, i64>(5)? as u64,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(GraphError::from)
    }

    /// Resolve a name or bare symbol-id to every matching current symbol-id, folding in alias
    /// history.
    pub fn symbol_ids_for(&self, project_id: &str, symbol: &str) -> Result<Vec<String>, GraphError> {
        let mut ids = HashSet::new();
        let mut stmt = self
            .conn
            .prepare("SELECT symbol_id FROM symbols WHERE project_id=?1 AND (name=?2 OR symbol_id=?2)")?;
        for row in stmt.query_map(params![project_id, symbol], |r| r.get::<_, String>(0))? {
            ids.insert(row?);
        }
        let mut alias_stmt = self
            .conn
            .prepare("SELECT symbol_id FROM aliases WHERE project_id=?1 AND name=?2")?;
        for row in alias_stmt.query_map(params![project_id, symbol], |r| r.get::<_, String>(0))? {
            ids.insert(row?);
        }
        let mut result: Vec<String> = ids.into_iter().collect();
        result.sort();
        Ok(result)
    }
}
