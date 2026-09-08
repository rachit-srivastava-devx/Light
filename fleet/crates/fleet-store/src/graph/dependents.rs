//! `dependents()`: the recursive-CTE "who transitively calls this" query, `graph.rs:1202-1249`.

use rusqlite::types::Value as SqlValue;
use rusqlite::params_from_iter;

use super::batch::DependentRecord;
use super::types::GraphError;
use super::GraphStore;

impl GraphStore {
    /// The recursive "who (transitively) calls any of `target_ids`" closure, bounded by `depth`.
    /// `depth == 0` is a typed error, not a silently empty result.
    pub fn dependents(
        &self,
        project_id: &str,
        target_ids: &[String],
        depth: u64,
    ) -> Result<Vec<DependentRecord>, GraphError> {
        if depth == 0 {
            return Err(GraphError::BadDepth(0));
        }
        let placeholders = target_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "WITH RECURSIVE closure(dependent_id, depth) AS (
               SELECT caller_id, 1 FROM edges WHERE project_id=? AND callee_id IN ({placeholders})
               UNION
               SELECT edges.caller_id, closure.depth + 1
               FROM edges JOIN closure ON edges.callee_id = closure.dependent_id
               WHERE edges.project_id=? AND closure.depth < ?
             )
             SELECT symbols.symbol_id, symbols.path, symbols.name, symbols.arity, symbols.kind, symbols.line, MIN(closure.depth)
             FROM closure JOIN symbols ON symbols.project_id=? AND symbols.symbol_id=closure.dependent_id
             GROUP BY symbols.symbol_id, symbols.path, symbols.name, symbols.arity, symbols.kind, symbols.line
             ORDER BY MIN(closure.depth), symbols.path, symbols.name, symbols.arity"
        );
        let mut values = vec![SqlValue::Text(project_id.to_string())];
        values.extend(target_ids.iter().cloned().map(SqlValue::Text));
        values.push(SqlValue::Text(project_id.to_string()));
        values.push(SqlValue::Integer(depth as i64));
        values.push(SqlValue::Text(project_id.to_string()));
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(values), |row| {
            Ok(DependentRecord {
                symbol_id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                arity: row.get::<_, i64>(3)? as u64,
                kind: row.get(4)?,
                line: row.get::<_, i64>(5)? as u64,
                depth: row.get::<_, i64>(6)? as u64,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(GraphError::from)
    }
}
