//! `LifecycleState` vocabulary. Lifted from `fleet/keel/fleet/src/lifecycle.rs:16-33` -- data
//! only, not the `Task<S>` machine.

use serde::{Deserialize, Serialize};

/// The 16 lifecycle states a fleet task passes through, as plain serializable data.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LifecycleState {
    Intake,
    Specified,
    Reviewed,
    Decomposed,
    Contracted,
    Briefed,
    Leased,
    Building,
    Built,
    Verifying,
    Verified,
    Attested,
    Accepted,
    Proposed,
    Observed,
    Refused,
}

/// The wire name + allowed-next table, indexed in declaration order.
const NAMES: [&str; 16] = [
    "Intake", "Specified", "Reviewed", "Decomposed", "Contracted", "Briefed", "Leased", "Building",
    "Built", "Verifying", "Verified", "Attested", "Accepted", "Proposed", "Observed", "Refused",
];

impl LifecycleState {
    /// The wire name, matching `lifecycle.rs`'s state names.
    pub fn name(self) -> &'static str {
        NAMES[self as usize]
    }

    /// The states legal to transition to from `self`. Mirrors `lifecycle.rs`'s `STATES` table
    /// verbatim; a total function over all 16 states.
    pub fn allowed_next(self) -> &'static [LifecycleState] {
        use LifecycleState::*;
        match self {
            Intake => &[Specified, Refused],
            Specified => &[Reviewed, Refused],
            Reviewed => &[Decomposed],
            Decomposed => &[Contracted],
            Contracted => &[Briefed],
            Briefed => &[Leased],
            Leased => &[Building],
            Building => &[Built, Refused],
            Built => &[Verifying, Refused],
            Verifying => &[Verified, Refused],
            Verified => &[Attested],
            Attested => &[Accepted, Refused],
            Accepted => &[Proposed, Refused],
            Proposed => &[Observed],
            Observed => &[Intake],
            Refused => &[],
        }
    }
}
