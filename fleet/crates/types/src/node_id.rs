//! `NodeId` -- module/leaf brief identifier. Lifted from
//! `contracts/module-brief.v1.json:138`.

use serde::{Deserialize, Serialize};

/// A `node_id` that did not match `^[a-z0-9][a-z0-9-]{2,63}$`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("node_id must match ^[a-z0-9][a-z0-9-]{{2,63}}$")]
pub struct BadNodeId;

/// Stable identifier for a module/leaf brief: lowercase alphanumeric + hyphen, 3-64 characters,
/// must not start with a hyphen.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(String);

fn matches_node_id_pattern(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() < 3 || bytes.len() > 64 {
        return false;
    }
    let first_ok = matches!(bytes[0], b'a'..=b'z' | b'0'..=b'9');
    first_ok && bytes.iter().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
}

impl NodeId {
    pub fn parse(value: impl Into<String>) -> Result<Self, BadNodeId> {
        let value = value.into();
        if !matches_node_id_pattern(&value) {
            return Err(BadNodeId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
