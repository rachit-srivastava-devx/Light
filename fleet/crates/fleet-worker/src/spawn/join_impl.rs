//! `join`: wait for the process, drain fd-3, tear down the worktree + sandbox unconditionally,
//! and record the scorecard outcome.

use super::interpret::interpret_fd3;
use super::process_group::{wait_with_deadline, WaitOutcome};
use crate::outcome::LaneOutcome;
use crate::request::{JoinError, LaneHandle};
use crate::{scorecard_io, ScorecardOutcome};

pub fn join(mut handle: LaneHandle) -> Result<LaneOutcome, JoinError> {
    let agent_id = lane_agent_id(&handle.worktree.name);

    let outcome = match wait_with_deadline(&mut handle.child, handle.deadline) {
        WaitOutcome::TimedOut => {
            unsafe { libc::close(handle.parent_fd) };
            LaneOutcome::EnvironmentFault {
                detail: "lane exceeded its deadline and was killed".into(),
            }
        }
        WaitOutcome::Exited => interpret_fd3(handle.parent_fd),
    };

    let score_outcome = match &outcome {
        LaneOutcome::Done { .. } => ScorecardOutcome::Credited,
        _ => ScorecardOutcome::Unknown,
    };
    let state_dir = handle.repo.join(".fleet").join("state");
    let _ = scorecard_io::record_scorecard_outcome(&state_dir, &agent_id, score_outcome);

    let _ = std::fs::remove_dir_all(&handle.sandbox_root);
    fleet_merge::remove(&handle.repo, &handle.worktree)
        .map_err(|e| JoinError::TeardownFailed(e.to_string()))?;
    Ok(outcome)
}

/// `fleet_merge::unique_name` builds `"<label>-<pid>-<seq>"`; the label is the role name this
/// lane was spawned for, which doubles as its scorecard key.
fn lane_agent_id(worktree_name: &str) -> String {
    let mut parts: Vec<&str> = worktree_name.rsplitn(3, '-').collect();
    parts.reverse();
    parts.first().copied().unwrap_or("unknown").to_string()
}

#[cfg(test)]
mod tests {
    use super::lane_agent_id;

    #[test]
    fn lane_agent_id_strips_pid_and_sequence() {
        assert_eq!(lane_agent_id("builder-12345-7"), "builder");
    }
}
