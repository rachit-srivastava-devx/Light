//! Retrieval and compaction on top of a repo map. No persistence, no LLM calls, no filesystem
//! walk, no network I/O -- every fact about the world arrives through a parameter or an injected
//! trait the caller supplies. See `blueprints/fleet-context/BLUEPRINT.md`.
//!
//! v1 scope (per MIGRATION-PLAN adjudication): tantivy BM25 + tree-sitter repo map + PageRank +
//! a tiktoken-rs token-budget compactor. `fastembed`/`ort` are NOT depended on; `VectorIndex` is
//! a port left unimplemented in this crate (see `embed.rs`) -- hybrid fusion degrades to
//! BM25-only until a later pass wires a real embedder.

mod bm25;
mod bm25_search;
mod compact;
mod conventions;
mod embed;
mod error;
mod fuse;
mod pagerank;
mod parse;
mod repomap;
mod repomap_edges;
mod repomap_parse;
mod retrieve;
mod tokens;
mod types;

pub use bm25::{IndexDoc, IndexLocation, TantivyIndex};
pub use conventions::{
    discover_conventions, fold_conventions, ConventionDoc, ConventionFold, ConventionFs,
    ConventionSet, DocKind, PlacedConventionDoc, StdConventionFs, TrimmedConventionDoc,
};
pub use compact::{compact_to_budget, Summarizer};
pub use embed::{NoVectorIndex, VectorIndex};
pub use error::ContextError;
pub use fuse::fuse_rrf;
pub use pagerank::pagerank;
pub use parse::language_for;
pub use repomap::build_repo_map;
pub use retrieve::{retrieve_context, RetrievalQuery};
pub use tokens::{count_tokens, TokenModel};
pub use types::{
    ContextSlice, Language, PlacedChunk, RepoMap, ScoredChunk, SourceFile, SymbolId, SymbolRef,
};
