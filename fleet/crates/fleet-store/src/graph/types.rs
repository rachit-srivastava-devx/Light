//! Record types + `GraphError`, mirroring `graph.rs`'s `projects/files/symbols/edges/aliases`.

use std::path::PathBuf;

use crate::io_fault::IoFault;

#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error(transparent)]
    Io(#[from] IoFault),
    #[error("sqlite: {0}")]
    Sql(String),
    #[error("depth must be >= 1, got {0}")]
    BadDepth(u64),
}

impl From<rusqlite::Error> for GraphError {
    fn from(e: rusqlite::Error) -> Self {
        GraphError::Sql(e.to_string())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectRecord {
    pub project_id: String,
    pub root_path: PathBuf,
    pub tree_digest: String,
    pub indexed_commit: String,
    pub indexed_file_count: u64,
    pub floor: u64,
    pub files_skipped: u64,
    pub languages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileRecord {
    pub path: String,
    pub language: String,
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SymbolRecord {
    pub symbol_id: String,
    pub path: String,
    pub name: String,
    pub arity: u64,
    pub kind: String,
    pub line: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeRecord {
    pub caller_id: String,
    pub callee_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AliasRecord {
    pub path: String,
    pub name: String,
    pub arity: u64,
    pub from_commit: String,
    pub to_commit: String,
    pub symbol_id: String,
}

