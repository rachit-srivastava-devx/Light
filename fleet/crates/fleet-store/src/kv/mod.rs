//! A generic redb-backed embedded KV for small state that needs neither a JSONL chain nor a SQL
//! schema. Greenfield -- see BLUEPRINT.md §7 for the redb choice.

mod store;
mod types;

pub use store::KvStore;
pub use types::KvError;
