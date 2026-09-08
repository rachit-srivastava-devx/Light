//! Honesty check between "the worker returned `done`" and "the worker actually changed the
//! worktree" -- the D-worst-defect fix: a chat-only adapter (no fenced-code-apply step) can
//! return prose that reads like success with zero diff. Called from `join` AFTER
//! `.fleet-sandbox/` is removed (so this crate's own scaffolding never needs pattern-matching
//! out of `git status`) and BEFORE the worktree itself is torn down -- this is the only window
//! where the real diff still exists on disk.
//!
//! "Changed" means: the worktree ended up different from `base_commit` (`HEAD` captured the
//! moment `fleet_merge::create` made this worktree) -- via an uncommitted diff OR a moved
//! `HEAD`. A worker that does real work and COMMITS it used to be missed entirely (`git status`
//! alone reports a clean tree for committed work), wrongly downgrading a genuine `Done` to
//! `Refused`; comparing `HEAD` against `base_commit` catches that case too. A git failure
//! (status or rev-parse, e.g. `git` missing from `PATH`) is never folded into "0 changes, so
//! refuse" -- that used to turn an ENVIRONMENT fault into a false `Refused` (AGENTS.md rule 7);
//! it now surfaces as `LaneOutcome::EnvironmentFault`, same as a malformed fd-3 packet.

mod detect;
mod error;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_committed;

use crate::outcome::LaneOutcome;
use detect::lane_changed;
use std::path::Path;

/// If `outcome` is `Done` but the worktree shows no real change relative to `base_commit`,
/// downgrade it to `Refused`. If change detection itself cannot run (a `git` failure), the
/// outcome becomes `EnvironmentFault` instead -- that is a fault in the check, not a claim
/// about what the worker did. Every other outcome (already `Refused`/`EnvironmentFault`) passes
/// through untouched.
pub fn enforce_change_honesty(outcome: LaneOutcome, worktree: &Path, base_commit: &str) -> LaneOutcome {
    let LaneOutcome::Done { body, .. } = &outcome else { return outcome };
    match lane_changed(worktree, base_commit) {
        Ok(true) => outcome,
        Ok(false) => LaneOutcome::Refused {
            reason: format!(
                "worker reported done but left the worktree unchanged relative to its starting \
                 commit {base_commit} -- the adapter returned advice, not an applied change. \
                 worker response: {body}"
            ),
        },
        Err(err) => LaneOutcome::EnvironmentFault { detail: err.to_string() },
    }
}
