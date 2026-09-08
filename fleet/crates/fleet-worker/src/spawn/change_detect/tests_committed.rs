//! The committed-work + moved-`HEAD` cases -- split out of `tests.rs` to keep that file under
//! the line budget.

use super::enforce_change_honesty;
use super::tests::{git, init_repo_with_commit};
use crate::outcome::LaneOutcome;
use serde_json::json;
use std::process::Command;

/// S1's exact repro: the worker performs real work AND COMMITS it, leaving `git status` clean.
/// Comparing final `HEAD` against `base_commit` must still see this as changed.
#[test]
fn a_committed_change_is_still_reported_as_done() {
    let dir = tempfile::tempdir().unwrap();
    let base = init_repo_with_commit(dir.path());
    std::fs::write(dir.path().join("f.txt"), "worker wrote this").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "worker change"]);
    // The mirror-image: without the fix, `git status --porcelain` alone is 0 lines here.
    let status = Command::new("git").arg("-C").arg(dir.path()).args(["status", "--porcelain"]).output().unwrap();
    assert!(status.stdout.is_empty(), "sanity: a committed change leaves git status clean");
    let outcome = LaneOutcome::Done { resolved_model: None, tokens: None, body: json!({}) };
    assert!(matches!(enforce_change_honesty(outcome, dir.path(), &base), LaneOutcome::Done { .. }));
}

/// A worker that commits, then resets back onto `base_commit` (amend/reset landing back on the
/// start point) nets to the same tree it started with -- must read as unchanged, same as a
/// true no-op, not as "changed" just because `HEAD` moved at some point mid-run.
#[test]
fn committing_then_resetting_back_to_base_is_still_refused() {
    let dir = tempfile::tempdir().unwrap();
    let base = init_repo_with_commit(dir.path());
    std::fs::write(dir.path().join("f.txt"), "temporary").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "temp"]);
    git(dir.path(), &["reset", "--hard", &base]);
    let outcome = LaneOutcome::Done { resolved_model: None, tokens: None, body: json!({}) };
    let out = enforce_change_honesty(outcome, dir.path(), &base);
    assert!(matches!(out, LaneOutcome::Refused { .. }), "{out:?}");
}

/// A worker that commits and leaves `HEAD` on a detached checkout (rather than the branch
/// `create` put it on) is still correctly read via `rev-parse HEAD`, which resolves a detached
/// `HEAD` to its commit sha exactly like an attached one.
#[test]
fn a_committed_change_on_a_detached_head_is_still_reported_as_done() {
    let dir = tempfile::tempdir().unwrap();
    let base = init_repo_with_commit(dir.path());
    std::fs::write(dir.path().join("f.txt"), "worker wrote this").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "worker change"]);
    let head = Command::new("git").arg("-C").arg(dir.path()).args(["rev-parse", "HEAD"]).output().unwrap();
    let head = String::from_utf8_lossy(&head.stdout).trim().to_string();
    git(dir.path(), &["checkout", "-q", "--detach", &head]);
    let outcome = LaneOutcome::Done { resolved_model: None, tokens: None, body: json!({}) };
    assert!(matches!(enforce_change_honesty(outcome, dir.path(), &base), LaneOutcome::Done { .. }));
}
