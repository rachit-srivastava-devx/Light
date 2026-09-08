//! `LoopPlan`/`LoopProgress` -- the durable shape of a multi-day autonomous run: a fixed ordered
//! sequence of work units (one feature/module per session) plus how far a prior run got.

use fleet_router::TaskClass;
use fleet_types::{Role, Tokens};

/// Identifies one unit of work in a `LoopPlan` (one feature/module the loop builds per session).
/// A thin `String` newtype, local to this crate -- not a `fleet_types` identifier, since a unit
/// is caller-defined work, not a lane or task lifecycle object.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct UnitId(String);

impl UnitId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A stable, ordered sequence of work units plus the routing/budget parameters `tick` uses to
/// pick a provider and admit tokens for whichever unit is next. Immutable for the run's lifetime
/// -- `LoopProgress` (persisted separately, via `LoopStore`) is the only thing that changes.
#[derive(Clone, Debug)]
pub struct LoopPlan {
    /// Stable key `LoopStore` persists progress under -- distinct plans never collide.
    pub id: String,
    pub units: Vec<UnitId>,
    pub role: Option<Role>,
    pub class: TaskClass,
    /// Token budget `admit`/`next_provider` estimate for one unit. A fixed per-unit estimate
    /// (never a float ratio, never a chars/4 guess) -- callers that want per-unit real counts can
    /// build this via `estimate()` before constructing the plan.
    pub tokens_per_unit: Tokens,
}

/// How far a `LoopPlan` has gotten. `completed.len()` is the index of the next unit to run --
/// this is the entire resume state a restart days later needs, and the only thing `LoopStore`
/// persists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoopProgress {
    pub completed: Vec<UnitId>,
}

/// The loop's persisted-progress file could not be read, decoded, or durably published.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct LoopIoError(pub String);
