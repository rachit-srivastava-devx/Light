//! The `Scorecard` data shape + its consistency invariant, ported verbatim from `agent.rs`.
//! The file-locked read-modify-write itself lives in `scorecard_io.rs` (kept separate to hold
//! this crate's 80-line-per-file rule).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Scorecard {
    pub agent_id: String,
    pub credited: u64,
    pub faulted: u64,
    pub unknown: u64,
    pub checked: u64,
    pub total: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScorecardOutcome {
    Credited,
    Faulted,
    Unknown,
}

impl Scorecard {
    pub(crate) fn empty(agent_id: &str) -> Self {
        Self { agent_id: agent_id.to_string(), credited: 0, faulted: 0, unknown: 0, checked: 0, total: 0 }
    }

    /// `checked == credited + faulted` and `total == checked + unknown` -- a corrupt scorecard
    /// on disk must be rejected on load, never silently accepted.
    pub(crate) fn consistent(&self) -> bool {
        self.checked == self.credited.saturating_add(self.faulted)
            && self.total == self.checked.saturating_add(self.unknown)
    }

    pub(crate) fn apply(&mut self, outcome: ScorecardOutcome) {
        match outcome {
            ScorecardOutcome::Credited => self.credited += 1,
            ScorecardOutcome::Faulted => self.faulted += 1,
            ScorecardOutcome::Unknown => self.unknown += 1,
        }
        self.checked = self.credited + self.faulted;
        self.total = self.checked + self.unknown;
    }
}

#[cfg(test)]
#[path = "scorecard_tests.rs"]
mod tests;
