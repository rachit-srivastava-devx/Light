use crate::merge::merge_lane;
use crate::merge_outcome::MergeOutcome;
use crate::worktree::{create as create_worktree, remove as remove_worktree, Worktree};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use types::{Blueprint, Module};

/// A lane represents one module's execution context with its own worktree and branch.
#[derive(Clone, Debug)]
pub struct Lane {
    /// Module ID
    pub module_id: String,
    /// Worktree for this lane
    pub worktree: Worktree,
    /// Module blueprint
    pub blueprint: Option<Blueprint>,
}

/// Outcome of a single lane's execution.
#[derive(Clone, Debug)]
pub struct LaneOutcome {
    /// Module ID
    pub module_id: String,
    /// Merge outcome if successful
    pub merge_outcome: Option<MergeOutcome>,
    /// Error if failed
    pub error: Option<LaneManagerError>,
}

/// Lane manager for parallel worktree orchestration.
/// Manages multiple module lanes with dependency-aware execution.
#[derive(Clone, Debug)]
pub struct LaneManager {
    /// Path to the git repository
    repo: PathBuf,
    /// Path to the state directory
    state_dir: PathBuf,
    /// Maximum number of concurrent lanes
    capacity: usize,
}

impl LaneManager {
    /// Create a new lane manager.
    ///
    /// # Arguments
    /// * `repo` - Path to the git repository
    /// * `state_dir` - Path to the state directory for tracking execution
    /// * `capacity` - Maximum number of parallel lanes to execute
    pub fn new(repo: PathBuf, state_dir: PathBuf, capacity: usize) -> Self {
        Self {
            repo,
            state_dir,
            capacity,
        }
    }

    /// Create worktrees for multiple modules in parallel.
    /// Returns a map of module_id to Lane.
    pub async fn create_worktrees(
        &self,
        modules: &[Module],
    ) -> Result<HashMap<String, Lane>, LaneManagerError> {
        let mut lanes = HashMap::new();

        for module in modules {
            let name = format!("module-{}-{}", module.id, std::process::id());
            let worktree = create_worktree(&self.repo, &name)
                .map_err(|e| LaneManagerError::WorktreeCreationFailed(e.to_string()))?;

            let lane = Lane {
                module_id: module.id.clone(),
                worktree,
                blueprint: None,
            };

            lanes.insert(module.id.clone(), lane);
        }

        Ok(lanes)
    }

    /// Update a lane with its blueprint.
    pub fn with_blueprint(
        &mut self,
        lanes: &mut HashMap<String, Lane>,
        module_id: &str,
        blueprint: Blueprint,
    ) {
        if let Some(lane) = lanes.get_mut(module_id) {
            lane.blueprint = Some(blueprint);
        }
    }

    /// Spawn lanes for modules in parallel, respecting the capacity limit.
    /// Returns outcomes for each lane.
    pub async fn spawn_lanes(
        &self,
        lanes: &mut HashMap<String, Lane>,
    ) -> Result<Vec<LaneOutcome>, LaneManagerError> {
        let mut outcomes = Vec::new();

        // Get module IDs in dependency order
        let sorted_ids: Vec<String> = lanes.keys().cloned().collect();

        // Process in batches based on capacity
        for chunk in sorted_ids.chunks(self.capacity) {
            let futures: Vec<_> = chunk
                .iter()
                .map(|module_id| {
                    let lane = lanes.get(module_id).unwrap().clone();
                    self.run_lane(lane)
                })
                .collect();

            let batch_outcomes = futures::future::join_all(futures).await;
            for outcome in batch_outcomes {
                outcomes.push(outcome?);
            }
        }

        Ok(outcomes)
    }

    /// Run a single lane: stage, commit, and merge.
    async fn run_lane(&self, lane: Lane) -> Result<LaneOutcome, LaneManagerError> {
        let module_id = lane.module_id.clone();
        let worktree_path = lane.worktree.path.clone();
        let branch = lane.worktree.branch.clone();

        match merge_lane(&self.repo, &worktree_path, &branch) {
            Ok(outcome) => Ok(LaneOutcome {
                module_id,
                merge_outcome: Some(outcome),
                error: None,
            }),
            Err(e) => Ok(LaneOutcome {
                module_id,
                merge_outcome: None,
                error: Some(LaneManagerError::MergeFailed(e)),
            }),
        }
    }

    /// Merge multiple lanes to main in dependency order.
    pub async fn merge_lanes_to_main(
        &self,
        lanes: &[Lane],
    ) -> Result<Vec<MergeOutcome>, LaneManagerError> {
        let mut outcomes = Vec::new();

        // Process lanes in order (they should already be sorted by dependencies)
        for lane in lanes {
            let outcome = merge_lane(&self.repo, &lane.worktree.path, &lane.worktree.branch)?;
            outcomes.push(outcome);
        }

        Ok(outcomes)
    }

    /// Clean up worktrees after execution.
    pub async fn cleanup_lanes(&self, lanes: &[Lane]) -> Result<(), LaneManagerError> {
        for lane in lanes {
            remove_worktree(&self.repo, &lane.worktree)
                .map_err(|e| LaneManagerError::WorktreeRemovalFailed(e.to_string()))?;
        }
        Ok(())
    }

    /// Get the state directory path.
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// Get the repository path.
    pub fn repo(&self) -> &Path {
        &self.repo
    }

    /// Get the capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Error types for lane manager operations.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LaneManagerError {
    /// Failed to create worktree
    #[error("failed to create worktree: {0}")]
    WorktreeCreationFailed(String),
    /// Failed to merge lane
    #[error("failed to merge lane: {0}")]
    MergeFailed(#[from] crate::merge_refusal::MergeRefusal),
    /// Failed to remove worktree
    #[error("failed to remove worktree: {0}")]
    WorktreeRemovalFailed(String),
    /// Module not found
    #[error("module not found: {0}")]
    ModuleNotFound(String),
}

impl LaneManagerError {
    /// Map to fleet's exit-code contract.
    pub fn exit_code(&self) -> types::ExitCode {
        match self {
            LaneManagerError::WorktreeCreationFailed(_) => types::ExitCode::Env,
            LaneManagerError::MergeFailed(_) => types::ExitCode::Invariant,
            LaneManagerError::WorktreeRemovalFailed(_) => types::ExitCode::Env,
            LaneManagerError::ModuleNotFound(_) => types::ExitCode::Invariant,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lane_creation() {
        let lane = Lane {
            module_id: "module-1".to_string(),
            worktree: Worktree {
                path: PathBuf::from("/tmp/test"),
                branch: "fleet/module-1".to_string(),
                name: "module-1-12345-0".to_string(),
            },
            blueprint: None,
        };

        assert_eq!(lane.module_id, "module-1");
        assert_eq!(lane.worktree.branch, "fleet/module-1");
    }

    #[test]
    fn test_lane_manager_creation() {
        let manager = LaneManager::new(PathBuf::from("/tmp/repo"), PathBuf::from("/tmp/state"), 4);

        assert_eq!(manager.repo().to_string_lossy(), "/tmp/repo");
        assert_eq!(manager.state_dir().to_string_lossy(), "/tmp/state");
        assert_eq!(manager.capacity(), 4);
    }

    #[test]
    fn test_lane_manager_error_exit_code() {
        let error = LaneManagerError::ModuleNotFound("test".to_string());
        assert_eq!(error.exit_code(), types::ExitCode::Invariant);
    }
}
