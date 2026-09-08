//! `Blake3Hash` -- the ledger wire format. `fleet/contracts/receipt.v1.json:12`.

use serde::{Deserialize, Serialize};

/// A string did not match `"blake3:" + 64 lowercase hex chars`.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0:?} is not a well-formed blake3 hash")]
pub struct BadBlake3Hash(pub String);

pub(crate) fn is_lowercase_hex(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// A blake3 digest in the ledger's wire format: `"blake3:"` followed by 64 lowercase hex chars.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Blake3Hash(String);

impl Blake3Hash {
    pub fn parse(value: impl Into<String>) -> Result<Self, BadBlake3Hash> {
        let value = value.into();
        match value.strip_prefix("blake3:") {
            Some(digest) if is_lowercase_hex(digest) => Ok(Self(value)),
            _ => Err(BadBlake3Hash(value)),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Blake3Hash {
    type Error = BadBlake3Hash;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<Blake3Hash> for String {
    fn from(hash: Blake3Hash) -> String {
        hash.0
    }
}
