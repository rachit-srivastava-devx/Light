//! The rusqlite code-graph store -- `graph.rs:612-694` (schema+reads), `:390-484` (writes).

mod batch;
mod dependents;
mod prune;
mod read;
mod schema;
mod types;
mod write;

pub use batch::{DependentRecord, ReindexBatch};
pub use types::{AliasRecord, EdgeRecord, FileRecord, GraphError, ProjectRecord, SymbolRecord};

pub struct GraphStore {
    pub(crate) conn: rusqlite::Connection,
    pub(crate) db_path: std::path::PathBuf,
}
