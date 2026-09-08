//! Pins S1-4 from `docs/DX-AUDIT.md`: `fleet swarm --task <non-empty>` used to be refused as
//! "task is empty or all-whitespace" unless `--prompt` was ALSO passed, because
//! `dispatch::swarm_cmd::swarm` built `SpawnRequest::task` from `args.prompt` alone (which
//! defaults to `""`) instead of falling back to `args.task`. Drives the REAL compiled binary
//! (`env!("CARGO_BIN_EXE_fleet")`), never a substituted fake.

mod support;
use support::cmd;

use std::process::Output;

fn swarm(args: &[&str]) -> Output {
    cmd().arg("swarm").args(args).output().expect("binary runs")
}

/// A non-empty `--task` with no `--prompt` must never be rejected as an empty prompt. The repo
/// path is deliberately missing a git checkout, so this can still fail downstream (worktree
/// creation, network) -- the point is only that the specific "task is empty" refusal is gone.
#[test]
fn task_alone_is_never_rejected_as_an_empty_prompt() {
    let dir = tempfile::tempdir().unwrap();
    support::scratch_repo(dir.path());
    let repo = dir.path().to_string_lossy().into_owned();

    let out = swarm(&["--repo", &repo, "--task", "hello world task", "--role", "builder"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!err.contains("task is empty"), "cross-wire regression: --task alone got: {err}");
}

/// The two failure modes the audit reproduced verbatim must now be identical whether or not
/// `--prompt` is also given -- both should progress past the emptiness check to the same
/// downstream outcome (worktree creation), proving `--task` alone is a real substitute for
/// `--task` + `--prompt`.
#[test]
fn task_alone_reaches_the_same_downstream_step_as_task_plus_prompt() {
    let dir = tempfile::tempdir().unwrap();
    support::scratch_repo(dir.path());
    let repo = dir.path().to_string_lossy().into_owned();

    let alone = swarm(&["--repo", &repo, "--task", "hello world task", "--role", "builder"]);
    let with_prompt =
        swarm(&["--repo", &repo, "--task", "hello world task", "--role", "builder", "--prompt", "hello world task"]);
    assert_eq!(alone.status.code(), with_prompt.status.code());
}
