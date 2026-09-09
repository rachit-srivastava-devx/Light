//! Opt-in merge-back after `join`'s own outcome is decided. Lives here, not in
//! `crates/fleet-merge/`: fleet-merge owns `merge_lane`'s invariants (staged/moved/changed), this
//! crate owns WHEN a lane's finished state warrants calling it at all.
//!
//! `MergePolicy::Never` is the default across every existing caller -- this module changes
//! nothing about `join`'s behaviour unless a caller explicitly opts in.

use crate::outcome::LaneOutcome;
use crate::request::JoinError;
use fleet_merge::MergeOutcome;
use std::path::Path;

/// Whether `join` should attempt to merge a finished lane's worktree branch back into the
/// target repo. Not a `bool`: a call site reading `join(handle, MergePolicy::OnSuccess)` says
/// what it means; `join(handle, true)` does not.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergePolicy {
    /// Never merge. Byte-identical to `join`'s behaviour before merge-back existed.
    Never,
    /// Merge iff the lane's outcome is `LaneOutcome::Done` (never `Refused`, including an
    /// honesty-downgraded one, and never `EnvironmentFault`).
    OnSuccess,
}

/// Attempt the merge iff `policy` opts in AND `outcome` is `Done`. MUST be called before
/// `fleet_merge::remove` -- that call deletes the worktree this reads from. Returns `Ok(None)`
/// (not an error) when policy/outcome don't call for a merge at all.
pub fn maybe_merge(
    policy: MergePolicy,
    outcome: &LaneOutcome,
    repo: &Path,
    worktree_dir: &Path,
    branch: &str,
) -> Result<Option<MergeOutcome>, JoinError> {
    if policy != MergePolicy::OnSuccess || !matches!(outcome, LaneOutcome::Done { .. }) {
        return Ok(None);
    }
    fleet_merge::merge_lane(repo, worktree_dir, branch)
        .map(Some)
        .map_err(|e| JoinError::MergeRefused(e.to_string()))
}
