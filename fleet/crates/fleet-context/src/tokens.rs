//! Real BPE token counts (tiktoken-rs) -- this crate's reason for existing is *not*
//! approximating a budget by character count. Encoders are cached (`OnceLock`) since building
//! a `CoreBPE`'s rank tables from scratch on every call is the dominant cost otherwise.

use std::sync::OnceLock;
use tiktoken_rs::{cl100k_base, o200k_base, CoreBPE};

/// The token model `count_tokens` counts against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenModel {
    Cl100kBase,
    O200kBase,
}

static CL100K: OnceLock<CoreBPE> = OnceLock::new();
static O200K: OnceLock<CoreBPE> = OnceLock::new();

/// Real BPE token count for `text` under `model`; pure, deterministic, no IO.
pub fn count_tokens(text: &str, model: TokenModel) -> u32 {
    let encoder = match model {
        TokenModel::Cl100kBase => CL100K.get_or_init(|| cl100k_base().expect("bundled cl100k ranks")),
        TokenModel::O200kBase => O200K.get_or_init(|| o200k_base().expect("bundled o200k ranks")),
    };
    encoder.encode_ordinary(text).len() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_cl100k_example() {
        // "hello world" is a well-known 2-token cl100k example.
        assert_eq!(count_tokens("hello world", TokenModel::Cl100kBase), 2);
    }

    #[test]
    fn empty_string_is_zero_tokens() {
        assert_eq!(count_tokens("", TokenModel::Cl100kBase), 0);
    }
}
