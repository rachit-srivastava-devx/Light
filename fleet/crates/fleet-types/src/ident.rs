//! `TaskId`, `LaneId`, and the shared `valid_artifact_id` predicate. Lifted from
//! `fleet/keel/fleet/src/lifecycle.rs:95-117`, `lane-status.v1.json:13-17`, and
//! `fleet/keel/fleet/src/main.rs:2271-2277`.

use serde::{Deserialize, Serialize};

/// A field that was empty or all-whitespace where a non-empty identifier was required.
///
/// Carries a preview of the offending input (truncated to 60 chars) and points
/// back at the grammar source so `fleet run --task <id>` / `fleet swarm --task <id>`
/// callers surface a message that names the rule that failed.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "{field} must not be empty or whitespace-only (got {input_preview}); \
     see fleet_types::{type_name}::parse for the full grammar"
)]
pub struct EmptyIdentifier {
    pub field: &'static str,
    pub type_name: &'static str,
    pub input_preview: String,
}

/// Render a user-supplied identifier for inclusion in an error message.
/// Wraps the value in double quotes so trailing whitespace is visible, and
/// truncates to 60 chars with an ellipsis marker when longer.
fn preview_input(value: &str) -> String {
    const MAX: usize = 60;
    if value.chars().count() <= MAX {
        format!("{value:?}")
    } else {
        let head: String = value.chars().take(MAX).collect();
        format!("{head:?} (truncated, {} chars total)", value.chars().count())
    }
}

/// Stable identifier carried through every lifecycle transition of one task.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    /// Grammar: any non-empty string that is not all whitespace.
    ///
    /// On refusal returns an [`EmptyIdentifier`] whose `Display` names the rule
    /// (`must not be empty or whitespace-only`), shows the offending input
    /// (truncated to 60 chars), and points at this function as the grammar source.
    pub fn parse(value: impl Into<String>) -> Result<Self, EmptyIdentifier> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(EmptyIdentifier {
                field: "task_id",
                type_name: "TaskId",
                input_preview: preview_input(&value),
            });
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
    /// Grammar: any non-empty string that is not all whitespace. See
    /// [`TaskId::parse`] for error shape.
    pub fn parse(value: impl Into<String>) -> Result<Self, EmptyIdentifier> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(EmptyIdentifier {
                field: "lane_id",
                type_name: "LaneId",
                input_preview: preview_input(&value),
            });
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
