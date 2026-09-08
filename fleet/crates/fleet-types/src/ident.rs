//! `TaskId`, `LaneId`, and the shared `valid_artifact_id` predicate. Lifted from
//! `fleet/keel/fleet/src/lifecycle.rs:95-117`, `lane-status.v1.json:13-17`, and
//! `fleet/keel/fleet/src/main.rs:2271-2277`.

use serde::{Deserialize, Serialize};

/// A field that was empty or all-whitespace where a non-empty identifier was required.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{field} must not be empty")]
pub struct EmptyIdentifier {
    pub field: &'static str,
}

/// Stable identifier carried through every lifecycle transition of one task.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    pub fn parse(value: impl Into<String>) -> Result<Self, EmptyIdentifier> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(EmptyIdentifier { field: "task_id" });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable identifier for a lane within one dispatch (today the role name).
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LaneId(String);

impl LaneId {
    pub fn parse(value: impl Into<String>) -> Result<Self, EmptyIdentifier> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(EmptyIdentifier { field: "lane_id" });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Shared artifact-id shape check: exactly 64 lowercase hex characters.
pub fn valid_artifact_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
