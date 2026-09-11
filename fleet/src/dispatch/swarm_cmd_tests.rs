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
        agent: "freelane".into(),
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

/// Unknown `--agent` values must be refused at dispatch, not silently defaulted to Freelane.
/// Uses a valid role + valid task so parse reaches the adapter step (unlike the env-var test
/// above, which fails earlier on an invalid role). `CliAdapter::from_agent_kind` is the single
/// parse point; this test proves swarm actually calls it and surfaces its typed error.
#[test]
fn unknown_agent_flag_is_refused_not_silently_defaulted() {
    let _guard = ENV_LOCK.lock().unwrap();
    let resolved = PathBuf::from("/tmp/fleet-swarm-agent-flag-test-state-dir");
    let args = SwarmArgs {
        repo: "/nonexistent".into(),
        task: "some-task".into(),
        role: "builder".into(), // valid role, see fleet_types::Role::ALL
        prompt: String::new(),
        merge: false,
        agent: "bogus".into(),
    };
    let result = swarm(&resolved, args);
    match result {
        Err(DispatchError::EnvFault(msg)) => {
            assert!(msg.contains("bogus"), "EnvFault must name the offending value: {msg}");
        }
        other => panic!("expected EnvFault for unknown --agent, got {other:?}"),
    }
}

/// Direct proof at the parse boundary: unknown agent kinds are typed errors, so `--agent bogus`
/// cannot slip through as a silent default (the whole point of Defect 1's fix).
#[test]
fn from_agent_kind_rejects_bogus_value() {
    assert!(fleet_worker::CliAdapter::from_agent_kind("bogus").is_err());
}
