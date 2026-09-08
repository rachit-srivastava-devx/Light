//! Tree-sitter parsing, ported from `graph.rs` (§5 of the blueprint) + this crate's own
//! deterministic `SymbolId`.

pub mod extract;
pub mod extract_call;
pub mod extract_definition;
pub mod language;
pub mod symbol_id;

pub use extract::{collect_nodes, RawCall};
pub use language::language_for;
