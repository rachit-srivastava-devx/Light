//! Attestation subject/digest types. `contracts/attestation.v1.json:10-20,28`.

use serde::{Deserialize, Serialize};

use crate::receipt::BadBlake3Hash;

/// `attestation.v1.json`'s `predicate.tier` enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DeliveryTier {
    #[serde(rename = "T-min")]
    TMin,
    #[serde(rename = "T-std")]
    TStd,
    #[serde(rename = "T-max")]
    TMax,
}

fn is_lowercase_hex(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// A 64-lowercase-hex blake3 digest with NO `"blake3:"` prefix -- distinct from `Blake3Hash`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BareBlake3Digest(String);

impl BareBlake3Digest {
    pub fn parse(value: impl Into<String>) -> Result<Self, BadBlake3Hash> {
        let value = value.into();
        if is_lowercase_hex(&value) {
            Ok(Self(value))
        } else {
            Err(BadBlake3Hash(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for BareBlake3Digest {
    type Error = BadBlake3Hash;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<BareBlake3Digest> for String {
    fn from(digest: BareBlake3Digest) -> String {
        digest.0
    }
}

/// `attestation.v1.json`'s `subject[]` entries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttestationSubject {
    pub name: String,
    pub digest: AttestationDigest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttestationDigest {
    pub blake3: BareBlake3Digest,
}
