//! Gap 2 regression: `swarm` must set `FLEET_STATE_DIR` from its OWN `state_dir` argument, not
//! merely happen to agree with whatever the env var already held. Proven by clearing the env var
//! first (so any pass would be from the threading, never a pre-set value) and using an invalid
//! role so `swarm` fails fast, offline, right after the env var is set -- no worktree, no
//! network. Serialized against other tests in this module via a process-wide mutex: env vars are
//! process-global and `cargo test` runs unit tests in one process by default.
use super::*;
use crate::cli::args_core::SwarmArgs;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn threads_resolved_state_dir_to_worker_even_when_env_var_was_unset() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var(ENV_STATE_DIR);
    let resolved = PathBuf::from("/tmp/fleet-swarm-gap2-test-state-dir");

    let args = SwarmArgs {
        repo: "/nonexistent".into(),
        task: "irrelevant".into(),
        role: "".into(), // invalid -> Role::parse fails, right after the env var is set
        prompt: String::new(),
        merge: false,
        then_verify: false,
    };
    let result = swarm(&resolved, args);

    assert!(result.is_err(), "invalid role must be refused, not silently accepted");
    assert_eq!(
        std::env::var(ENV_STATE_DIR).as_deref(),
        Ok("/tmp/fleet-swarm-gap2-test-state-dir"),
        "FLEET_STATE_DIR must be set from the CLI's resolved state_dir, not left to coincidence"
    );
    std::env::remove_var(ENV_STATE_DIR);
}

/// `--then-verify` on a successful lane (`Done`) must hand the verify pipeline the SAME
/// `--repo` and `--task` the swarm invocation received -- byte-identical. This is the "one
/// command instead of two" promise the flag exists to deliver, so a drift here (e.g. a mutated
/// or re-parsed path/task id) would silently violate the flag's whole reason for being.
#[test]
fn then_verify_on_done_hands_verify_the_same_repo_and_task() {
    let plan = verify_plan(true, true, "/some/repo", "TASK-42");
    assert_eq!(plan, Some(VerifyPlan { repo: "/some/repo".into(), task: "TASK-42".into() }));
}

/// `--then-verify` on a Refused/EnvironmentFault lane must NOT invoke the verify pipeline --
/// the whole point of the short-circuit is that chaining verify on a lane that never finished
/// is meaningless work (and would mask swarm's exit code with verify's). Pinned as the pair
/// `(swarm_done=false, plan=None)` because a `Some(_)` here would silently break the exit-code
/// contract documented on `SwarmArgs::then_verify`.
#[test]
fn then_verify_on_non_done_skips_verify() {
    assert_eq!(verify_plan(true, false, "/r", "T"), None);
}

/// Regression pin: absent `--then-verify`, verify must NEVER be invoked, regardless of whether
/// the lane finished `Done`. This is the "behaviour unchanged when flag is off" guarantee --
/// existing scripts and CI that call `fleet swarm` without the flag must see byte-identical
/// behaviour from this composition root.
#[test]
fn absent_then_verify_never_invokes_verify() {
    assert_eq!(verify_plan(false, true, "/r", "T"), None);
    assert_eq!(verify_plan(false, false, "/r", "T"), None);
}

/// Integration-adjacent: driving the full `swarm` fn with `--then-verify` and an invalid role
/// forces the pre-spawn Refusal path -- the lane never reaches `Done`, so the short-circuit
/// contract says verify must NOT be invoked and the exit code must be swarm's own Refusal.
/// If verify had been invoked, it would have tried to open `/nonexistent` as a repo and either
/// surfaced a DIFFERENT error string (masking the role Refusal) or panicked -- either is a
/// louder failure than this assertion. Serialized against the env-var test above via the same
/// process-wide mutex.
#[test]
fn then_verify_with_refused_lane_returns_swarm_refusal() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::remove_var(ENV_STATE_DIR);
    let resolved = PathBuf::from("/tmp/fleet-swarm-then-verify-refusal-test");

    let args = SwarmArgs {
        repo: "/nonexistent".into(),
        task: "irrelevant".into(),
        role: "".into(), // invalid -> Role::parse fails, before any lane or verify runs
        prompt: String::new(),
        merge: false,
        then_verify: true, // must NOT chain verify on a Refusal
    };
    let result = swarm(&resolved, args);

    assert!(matches!(result, Err(DispatchError::Refusal(_))),
        "then-verify must preserve swarm's Refusal exit code, got: {result:?}");
    std::env::remove_var(ENV_STATE_DIR);
}
