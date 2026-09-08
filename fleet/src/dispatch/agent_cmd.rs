//! `__agent` -- the real child-side entry point `fleet-worker`'s `child_command::build` re-execs
//! into as `<exe> __agent <kind> <worktree> <task> [model]`. Fixes the bug where this subcommand
//! did not exist at all: every real spawned worker died instantly with "unrecognized subcommand
//! '__agent'" (all 351 prior tests passed anyway because they substitute a fake binary via
//! `FLEET_WORKER_TEST_CHILD_EXE`, never exercising this path -- PRINCIPLES: a proxy is not the
//! property). Ported in spirit from keel's `agent_command` (`git show
//! HEAD:fleet/keel/fleet/src/main.rs:3194`); see `agent_cmd_run.rs` for the scope narrowed to
//! what this migrated tree can actually execute, and `agent_cmd_error.rs` for the exit codes.

pub use super::agent_cmd_error::AgentCmdError;

use super::agent_cmd_run::{self, AgentOutcome};
use crate::cli::args_agent::AgentArgs;
use fleet_worker::{fd3, CliAdapter};

pub fn agent(args: AgentArgs) -> Result<(), AgentCmdError> {
    let adapter = CliAdapter::from_agent_kind(&args.kind)?;
    if !args.worktree.is_dir() {
        return Err(AgentCmdError::MissingWorktree(args.worktree));
    }
    if args.task.trim().is_empty() {
        return Err(AgentCmdError::EmptyTask);
    }
    if !fd3::child_channel_open() {
        return Err(AgentCmdError::Fd3NotOpen);
    }

    let outcome = agent_cmd_run::run(adapter, &args.worktree, &args.task, args.model.as_deref());
    match outcome {
        AgentOutcome::Done { body, resolved_model } => {
            fd3::send_done(body, resolved_model.as_deref(), None)?;
            Ok(())
        }
        AgentOutcome::Refused(reason) => {
            let _ = fd3::send_refuse(&reason, serde_json::json!({}));
            Err(AgentCmdError::Refused(reason))
        }
    }
}
