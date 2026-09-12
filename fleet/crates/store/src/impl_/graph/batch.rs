//! `ReindexBatch` (a caller-supplied re-index) and `DependentRecord` (a `dependents()` hit).

use super::types::{AliasRecord, EdgeRecord, FileRecord, ProjectRecord, SymbolRecord};

/// One project's full re-index, already computed by the caller. This crate never parses; it only
/// replaces one project's rows transactionally.
pub struct ReindexBatch {
    pub project: ProjectRecord,
    pub files: Vec<FileRecord>,
    pub symbols: Vec<SymbolRecord>,
    pub edges: Vec<EdgeRecord>,
    pub aliases: Vec<AliasRecord>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DependentRecord {
    pub symbol_id: String,
    pub path: String,
    pub name: String,
    pub arity: u64,
    pub kind: String,
    pub line: u64,
    pub depth: u64,
}
