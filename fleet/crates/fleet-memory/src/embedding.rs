//! `Embedding` and `CosineSimilarity`. See BLUEPRINT.md §3.B.

/// A dense embedding vector, non-empty by construction. Dimension is not hardcoded so a future
/// model change never requires a type change here.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Embedding(Vec<f32>);

/// `Embedding::new` was given an empty vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("embedding must have at least one dimension")]
pub struct EmptyEmbedding;

/// Two compared embeddings had different lengths — never silently zero-padded or truncated.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("embeddings have different dimensions: {a} vs {b}")]
pub struct DimensionMismatch {
    pub a: usize,
    pub b: usize,
}

/// Cosine similarity, clamped to `[-1.0, 1.0]` by construction.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct CosineSimilarity(f64);

impl Embedding {
    pub fn new(values: Vec<f32>) -> Result<Self, EmptyEmbedding> {
        if values.is_empty() {
            return Err(EmptyEmbedding);
        }
        Ok(Self(values))
    }

    pub fn dim(&self) -> usize {
        self.0.len()
    }

    /// Pure dot-product-over-norms computation; never IO, never a model call. An all-zero
    /// embedding on either side has an undefined direction, so this returns `0.0` similarity
    /// rather than dividing by zero into `NaN` (not typed as an error in BLUEPRINT.md §3).
    pub fn cosine(&self, other: &Embedding) -> Result<CosineSimilarity, DimensionMismatch> {
        if self.0.len() != other.0.len() {
            return Err(DimensionMismatch { a: self.0.len(), b: other.0.len() });
        }
        let dot: f64 = self.0.iter().zip(&other.0).map(|(a, b)| *a as f64 * *b as f64).sum();
        let norm_a: f64 = self.0.iter().map(|v| (*v as f64).powi(2)).sum::<f64>().sqrt();
        let norm_b: f64 = other.0.iter().map(|v| (*v as f64).powi(2)).sum::<f64>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return Ok(CosineSimilarity(0.0));
        }
        Ok(CosineSimilarity((dot / (norm_a * norm_b)).clamp(-1.0, 1.0)))
    }
}

impl CosineSimilarity {
    pub fn get(self) -> f64 {
        self.0
    }
}
