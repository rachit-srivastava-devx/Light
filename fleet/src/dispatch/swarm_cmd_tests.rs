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

// The human-path `spawned` line must name the CLI adapter driving the lane so a script or a
// human reader can tell freelane apart from claude apart from codex at a glance -- the plain
// "spawned" line looks identical across adapters otherwise, which becomes actively confusing
// once `--agent` picks a non-default. The format is additive: `spawned` stays the leading
// token so any downstream regex on `[lane] spawned` keeps matching; `agent=<kind>` is
// appended, and the `<kind>` string is exactly `CliAdapter::agent_kind()` -- never invented.
#[test]
fn spawned_line_appends_adapter_kind_from_cli_adapter_agent_kind() {
    for adapter in [CliAdapter::Freelane, CliAdapter::Claude, CliAdapter::Codex] {
        let kind = adapter.agent_kind();
        let line = spawned_line(kind);
        assert!(line.starts_with("spawned "), "prefix must stay `spawned ` (line={line:?})");
        assert_eq!(line, format!("spawned agent={kind}"));
        assert!(line.contains(&format!("agent={kind}")), "line must carry agent={kind}: {line:?}");
    }
}
