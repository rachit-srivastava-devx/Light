//! `send_done_committed` -- split out of `fixture_scenarios.rs` to keep that file under the
//! line budget.

use crate::fixture_scenarios::send_done;

/// Same `done` packet as `send_done`, but writes a file AND COMMITS it inside `worktree` before
/// reporting done -- the S1 regression fixture: a worker that does real work and then commits
/// leaves `git status` clean, so `change_detect` must fall back to comparing `HEAD` against the
/// worktree's recorded base commit rather than trusting a clean `git status` alone.
pub fn send_done_committed(worktree: &str) {
    let _ = std::fs::write(format!("{worktree}/fixture-committed-change.txt"), "committed change\n");
    let git = |args: &[&str]| {
        let _ = std::process::Command::new("git").arg("-C").arg(worktree).args(args).status();
    };
    git(&["add", "-A"]);
    git(&[
        "-c",
        "user.email=fixture@example.com",
        "-c",
        "user.name=fixture",
        "commit",
        "-qm",
        "fixture: committed work",
    ]);
    send_done();
}
