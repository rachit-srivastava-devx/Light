use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("symbol not found: {0}")]
    NotFound(String),
    #[error("stale snapshot")]
    StaleSnapshot,
    #[error("reader unavailable")]
    Unavailable,
}

pub struct SymbolFact {
    pub query: String,
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub exists: bool,
}

pub trait CodebaseReader: Send + Sync {
    fn symbol(&self, query: &str) -> Result<SymbolFact, ProbeError>;
}
