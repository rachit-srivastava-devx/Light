//! `MemoryRow`/`Severity`/`Bm25Hit`/`VectorHit`/`MemoryError`.

use crate::io_fault::IoFault;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Critical,
    Important,
    Minor,
}

impl Severity {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Important => "important",
            Self::Minor => "minor",
        }
    }

    pub(super) fn parse(value: &str) -> Self {
        match value {
            "critical" => Self::Critical,
            "minor" => Self::Minor,
            _ => Self::Important,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MemoryRow {
    pub id: String,
    pub title: String,
    pub body: String,
    pub source: String,
    pub keywords: Vec<String>,
    pub evidence: String,
    pub severity: Severity,
}

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error(transparent)]
    Io(#[from] IoFault),
    #[error("sqlite: {0}")]
    Sql(String),
    #[error("embedding has {found} dimensions, expected {expected}")]
    DimensionMismatch { expected: u32, found: u32 },
}

impl From<rusqlite::Error> for MemoryError {
    fn from(e: rusqlite::Error) -> Self {
        MemoryError::Sql(e.to_string())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bm25Hit {
    pub id: String,
    pub source: String,
    pub title: String,
    pub severity: Severity,
    pub confirmed_count: u32,
    pub relevance: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VectorHit {
    pub id: String,
    pub distance: f32,
}
