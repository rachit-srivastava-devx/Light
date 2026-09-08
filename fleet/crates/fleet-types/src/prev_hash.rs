//! `PrevHash` -- `receipt.v1.json`'s `prev_hash`: either the ledger genesis marker or a prior
//! row's hash. Hand-written (de)serialize since the wire form is a plain string, not a nested
//! externally-tagged enum.

use serde::{Deserialize, Serialize};

use crate::receipt::{BadBlake3Hash, Blake3Hash};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrevHash {
    Genesis,
    Hash(Blake3Hash),
}

impl PrevHash {
    pub fn parse(value: impl Into<String>) -> Result<Self, BadBlake3Hash> {
        let value = value.into();
        if value == "GENESIS" {
            return Ok(Self::Genesis);
        }
        Blake3Hash::parse(value).map(Self::Hash)
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Genesis => "GENESIS",
            Self::Hash(hash) => hash.as_str(),
        }
    }
}

impl Serialize for PrevHash {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for PrevHash {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        PrevHash::parse(value).map_err(serde::de::Error::custom)
    }
}
