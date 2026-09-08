//! Fleet's durable state layer: ledger, code-graph, hybrid memory substrate, embedded KV.
//!
//! Every store here takes its filesystem path(s) from the caller at construction time -- this
//! crate never derives a path from an environment variable or a default directory. Every
//! fallible operation returns a typed, per-domain error enum. No store in this crate ranks,
//! scores, fuses, or decides anything about the rows it holds; it stores and retrieves them
//! exactly as given.

mod io_fault;
mod lock;

pub mod graph;
pub mod kv;
pub mod ledger;
pub mod memory;
pub mod retention;

pub use graph::GraphStore;
pub use io_fault::IoFault;
pub use kv::KvStore;
pub use ledger::Ledger;
pub use memory::MemoryStore;
pub use retention::{PruneReport, RetentionError, RetentionPolicy, UsageReport};
