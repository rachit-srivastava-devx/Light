//! Builds the child `Command`: `<current_exe> __agent <kind> <worktree> <task> [model]` under
//! the hermetic env, mirroring `main.rs::spawn_agent`'s arg shape.

use crate::request::{SpawnError, SpawnRequest};
use crate::sandbox::hermetic_env::HermeticEnv;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::util::CHILD_EXE_OVERRIDE;

pub fn build(request: &SpawnRequest, worktree_path: &Path, hermetic: &HermeticEnv) -> Result<Command, SpawnError> {
    let executable = std::env::var_os(CHILD_EXE_OVERRIDE)
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
        .ok_or_else(|| SpawnError::ProcessSpawnFailed("no current_exe".to_string()))?;
    let mut command = Command::new(executable);
    command.arg("__agent").arg(request.adapter.agent_kind());
    command.arg(worktree_path).arg(&request.task);
    if let Some(model) = &request.requested_model {
        command.arg(model);
    }
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    hermetic.apply(&mut command);
    Ok(command)
}
