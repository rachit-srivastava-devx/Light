//! `TaskId`. Ported from `fleet/keel/fleet/src/lifecycle.rs:96-117`.

use fleet_types::GateRefusal;
use std::fmt;

/// Stable identifier carried through every transition. Never empty.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskId(String);

impl TaskId {
    /// `Err(GateRefusal{code: "EMPTY_TASK_ID", ..})` iff `value.trim()` is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, GateRefusal> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(GateRefusal::new("EMPTY_TASK_ID", "a task id must not be empty"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
