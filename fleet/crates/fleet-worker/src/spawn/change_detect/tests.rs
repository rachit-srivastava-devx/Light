use super::enforce_change_honesty;
use crate::outcome::LaneOutcome;
use serde_json::json;
use std::process::Command;

pub(super) fn git(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git").arg("-C").arg(dir).args(args).status().expect("git");
    assert!(status.success(), "git {args:?} failed");
}

pub(super) fn init_repo_with_commit(dir: &std::path::Path) -> String {
    git(dir, &["init", "-q"]);
    git(dir, &["config", "user.email", "test@example.com"]);
    git(dir, &["config", "user.name", "test"]);
    std::fs::write(dir.join("base.txt"), "base").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "init"]);
    let out = Command::new("git").arg("-C").arg(dir).args(["rev-parse", "HEAD"]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A directory that was never `git init`'d at all -- `git status`/`git rev-parse HEAD` both
/// fail to find a repo. Per AGENTS.md rule 7 this is an ENVIRONMENT fault, not "the worker did
/// nothing": before this fix it was silently folded into `changed_files: 0` and downgraded to a
/// false `Refused`.
#[test]
fn git_failure_is_an_environment_fault_not_a_refusal() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = LaneOutcome::Done { resolved_model: None, tokens: None, body: json!({"ok": true}) };
    let out = enforce_change_honesty(outcome, dir.path(), "deadbeef");
    assert!(matches!(out, LaneOutcome::EnvironmentFault { .. }), "{out:?}");
}

#[test]
fn non_done_outcomes_pass_through_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = LaneOutcome::Refused { reason: "already refused".into() };
    let out = enforce_change_honesty(outcome, dir.path(), "deadbeef");
    assert!(matches!(out, LaneOutcome::Refused { reason } if reason == "already refused"));
}

/// MIRROR-IMAGE guard: a worker that leaves an uncommitted diff must still read as `Done`.
#[test]
fn an_uncommitted_change_is_still_reported_as_done() {
    let dir = tempfile::tempdir().unwrap();
    let base = init_repo_with_commit(dir.path());
    std::fs::write(dir.path().join("f.txt"), "worker wrote this").unwrap();
    let outcome = LaneOutcome::Done { resolved_model: None, tokens: None, body: json!({}) };
    assert!(matches!(enforce_change_honesty(outcome, dir.path(), &base), LaneOutcome::Done { .. }));
}

/// A chat-only worker that touches nothing at all: `git status` clean AND `HEAD` unmoved. Must
/// still be refused -- the fix must not weaken this, the original no-op case.
///
/// NOTE: this calls `enforce_change_honesty` directly against a bare `tempfile::tempdir()` that
/// never went through the real `spawn()`, so `record_worker_pid` never wrote `.fleet-lane.pid`
/// into it. It does NOT cover the real defect (fleet's own pid marker making every worktree look
/// dirty to `git status`) -- see `spawn_join_true_no_op_healthy_repo.rs` for that.
#[test]
fn a_true_no_op_is_still_refused() {
    let dir = tempfile::tempdir().unwrap();
    let base = init_repo_with_commit(dir.path());
    let outcome = LaneOutcome::Done { resolved_model: None, tokens: None, body: json!({}) };
    let out = enforce_change_honesty(outcome, dir.path(), &base);
    assert!(matches!(out, LaneOutcome::Refused { .. }), "{out:?}");
}
