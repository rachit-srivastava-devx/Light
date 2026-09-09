//! `fleet __spawn_probe`: the one real call site for `fleet_worker::spawn`/`join`'s parent-side
//! path from a process whose own `current_exe()` genuinely is `fleet` -- proving the fd-3 wire
//! round-trips through a REAL re-exec, not the `FLEET_WORKER_TEST_CHILD_EXE` fault-injection
//! seam (`src/tests/agent_child_dispatch.rs` is the test that drives this).

use crate::cli::args_agent::SpawnProbeArgs;
use crate::dispatch::error::DispatchError;
use fleet_types::{Role, TaskId};
use fleet_worker::{join, spawn, CliAdapter, LaneOutcome, MergePolicy, SpawnRequest};
use std::path::PathBuf;
use std::time::Duration;

pub fn probe(args: SpawnProbeArgs) -> Result<(), DispatchError> {
    let request = SpawnRequest {
        repo: PathBuf::from(args.repo),
        role: Role::Builder,
        task_id: TaskId::parse("spawn-probe").expect("static id is non-empty"),
        adapter: CliAdapter::Freelane,
        requested_model: None,
        task: "spawn-probe: prove the fd-3 receipt round-trips".to_string(),
        deadline: Duration::from_secs(30),
    };
    let handle = spawn(request)?;
    let (outcome, _merge) = join(handle, MergePolicy::Never)?;
    match outcome {
        LaneOutcome::Done { .. } => println!("__spawn_probe: done"),
        LaneOutcome::Refused { reason } => println!("__spawn_probe: refused: {reason}"),
        LaneOutcome::EnvironmentFault { detail } => {
            return Err(DispatchError::Refusal(format!(
                "__spawn_probe: no receipt came back over fd 3: {detail}"
            )))
        }
    }
    Ok(())
}
