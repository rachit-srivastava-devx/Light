//! Real token estimation -- never `chars/4`. `TiktokenTokenizer` wraps `tiktoken-rs`'s
//! `cl100k_base`/`o200k_base` BPE encoding; tests may inject a fixed-count fake instead.

use fleet_types::Tokens;

/// A loaded tokenizer. Pure (same text always yields the same count) but still a port, not
/// called directly, so a tokenizer-version bump is a caller decision, not a hidden constant.
pub trait Tokenizer: Send + Sync {
    fn count(&self, text: &str) -> Tokens;
}

/// Convenience wrapper so callers building a `cost_est` for `admit` never hand-roll a
/// `len() / 4` heuristic.
pub fn estimate(tokenizer: &dyn Tokenizer, text: &str) -> Tokens {
    tokenizer.count(text)
}

/// `tiktoken-rs`'s encoding could not be loaded (vocab is compiled in, so this is effectively
/// infallible in practice, but the loader is still fallible per its own signature).
#[derive(Debug, thiserror::Error)]
#[error("failed to load tiktoken encoding: {0}")]
pub struct TokenizerLoadError(String);

pub struct TiktokenTokenizer {
    bpe: tiktoken_rs::CoreBPE,
}

impl TiktokenTokenizer {
    pub fn cl100k_base() -> Result<Self, TokenizerLoadError> {
        tiktoken_rs::cl100k_base()
            .map(|bpe| Self { bpe })
            .map_err(|e| TokenizerLoadError(e.to_string()))
    }

    pub fn o200k_base() -> Result<Self, TokenizerLoadError> {
        tiktoken_rs::o200k_base()
            .map(|bpe| Self { bpe })
            .map_err(|e| TokenizerLoadError(e.to_string()))
    }
}

impl Tokenizer for TiktokenTokenizer {
    fn count(&self, text: &str) -> Tokens {
        Tokens::new(self.bpe.encode_ordinary(text).len() as u64)
    }
}
