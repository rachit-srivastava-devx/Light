//! Vector search port. **Scope note (v1 adjudication):** `fastembed`/`ort` are deliberately not
//! depended on -- `ort` has never cut a stable release and drags a large native ONNX binary
//! (blueprint §7's flagged risk). This crate defines the seam only; no concrete embedder ships in
//! this pass, so `retrieve_context` degrades to BM25-only until a later pass wires an embedder and
//! a `VectorIndex` implementation (`fleet-store`'s job -- this crate never implements it itself).

use crate::error::ContextError;
use crate::types::SymbolId;

/// A `SymbolId`-keyed nearest-neighbor query surface. This crate never implements this trait
/// itself; a later pass's `fleet-store` adapter does, wired in `src/`.
pub trait VectorIndex {
    fn upsert(&mut self, id: &SymbolId, vector: &[f32]) -> Result<(), ContextError>;
    /// Up to `k` `(SymbolId, cosine_similarity)` pairs, descending by similarity.
    fn nearest(&self, query_vector: &[f32], k: u32) -> Result<Vec<(SymbolId, f32)>, ContextError>;
}

/// A no-op `VectorIndex` used when no real vector backend is wired -- `nearest` always returns
/// empty, so `retrieve_context` degrades cleanly to BM25-only fusion (§2/§7 divergence note).
pub struct NoVectorIndex;

impl VectorIndex for NoVectorIndex {
    fn upsert(&mut self, _id: &SymbolId, _vector: &[f32]) -> Result<(), ContextError> {
        Ok(())
    }

    fn nearest(&self, _query_vector: &[f32], _k: u32) -> Result<Vec<(SymbolId, f32)>, ContextError> {
        Ok(Vec::new())
    }
}
