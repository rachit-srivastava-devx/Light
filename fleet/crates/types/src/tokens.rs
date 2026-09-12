//! `Tokens` -- a count of language-model tokens, always `u64`, never float, every combining
//! operation `checked_*`. New shared vocabulary (see BLUEPRINT.md §9 divergence note 4).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tokens(u64);

/// A `Tokens` arithmetic operation would have overflowed `u64`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("token arithmetic overflowed u64")]
pub struct TokensOverflow;

impl Tokens {
    pub const ZERO: Tokens = Tokens(0);

    pub fn new(value: u64) -> Self {
        Tokens(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }

    pub fn checked_add(self, rhs: Tokens) -> Result<Tokens, TokensOverflow> {
        self.0.checked_add(rhs.0).map(Tokens).ok_or(TokensOverflow)
    }

    pub fn checked_sub(self, rhs: Tokens) -> Result<Tokens, TokensOverflow> {
        self.0.checked_sub(rhs.0).map(Tokens).ok_or(TokensOverflow)
    }
}
