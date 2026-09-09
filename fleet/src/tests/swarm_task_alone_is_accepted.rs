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

// `fleet_types::ExitCode::Invariant` -- the code `SpawnError::EmptyTask` maps to (the fallback
// arm of `DispatchError::exit_code`). Duplicated as a plain const, matching
// `capacity_refusal_real_binary.rs`'s precedent, rather than pulling `fleet_types` into this
// integration-test crate's dependency graph for one constant.
const INVARIANT: i32 = 6;

/// The two invocations both spawn a REAL worker that makes a REAL call to a free, rate-limited
/// keyless endpoint (`api.llm7.io`) -- so their exit codes may legitimately differ (one call can
/// be throttled while the other succeeds; see `docs/DX-AUDIT.md` S1-4 follow-up). Asserting the
/// codes are EQUAL was too strong: it made the suite flake on rate limiting, for reasons that had
/// nothing to do with the regression this test exists to pin. Instead assert the actual property
/// on each invocation independently: neither was rejected as an empty/whitespace task (the S1-4
/// defect), which is the one thing `--prompt` being present-or-absent must never change.
#[test]
fn task_alone_reaches_the_same_downstream_step_as_task_plus_prompt() {
    let dir = tempfile::tempdir().unwrap();
    support::scratch_repo(dir.path());
    let repo = dir.path().to_string_lossy().into_owned();

    let alone = swarm(&["--repo", &repo, "--task", "hello world task", "--role", "builder"]);
    let with_prompt =
        swarm(&["--repo", &repo, "--task", "hello world task", "--role", "builder", "--prompt", "hello world task"]);

    for (label, out) in [("--task alone", &alone), ("--task + --prompt", &with_prompt)] {
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!err.contains("task is empty"), "{label}: rejected as empty task: {err}");
        assert_ne!(
            out.status.code(),
            Some(INVARIANT),
            "{label}: exited with the empty-task refusal code: {err}"
        );
    }
}
