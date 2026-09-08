//! Worktree lifecycle + merge-back invariants for fleet's parallel role lanes.
//!
//! This crate answers two questions, both IO-boundary-adjacent but each with a hard,
//! previously-violated-in-production invariant behind it: "did creating an isolated worktree
//! actually succeed and exist on disk" (`worktree` module) and "did merging a lane's branch
//! back actually land real work, not a silent no-op" (`merge` module; `invariant` holds the
//! three pure checks so they are unit-testable without a real git repo). Git itself is invoked
//! as a direct subprocess (`std::process::Command`) via `git_exec`, matching
//! `fleet/keel/fleet/src/worktree.rs` and `fleet/bin/merge-lane.sh`'s existing pattern exactly:
//! no `git2`/libgit2 dependency exists anywhere in the workspace's lockfile today, and a
//! trait-injected git boundary would buy no unit-test speed here -- the invariants this crate
//! exists to protect are properties of *real* git output, so every integration test runs
//! against a real temp repo, never a mock.
//!
//! Thread-safety: every public fn takes owned/borrowed values and returns owned values with no
//! interior mutability. Concurrency safety for `create`/`remove`/`merge_lane` against the *same*
//! repo relies on git's own `.git/index.lock`, not a lock this crate adds -- exactly as
//! `worktree.rs`/`merge-lane.sh` behave today.

mod cache;
mod error;
mod git_exec;
mod invariant;
mod merge;
mod merge_refusal;
mod worktree;
mod worktree_guard;

pub use cache::invalidate_build_cache;
pub use error::{MergeRefusal, WorktreeError};
pub use invariant::{check_files_changed, check_head_moved, check_stage_nonempty};
pub use merge::{merge_lane, MergeOutcome};
pub use worktree::{create, remove, unique_name, Worktree};
