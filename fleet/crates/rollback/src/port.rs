use std::path::Path;

use crate::{ArtifactRecord, RollbackError, RollbackReceipt};

pub trait RollbackStore {
    fn get_artifact(&self, id: &str) -> Option<ArtifactRecord>;
    fn write_receipt(&mut self, receipt: &RollbackReceipt);
    fn worktrees_root(&self) -> &Path;
}

pub trait GitRollbackPort {
    fn remove_owned(&self, path: &Path) -> Result<(), RollbackError>;
    fn revert(&self, repo: &Path, commit: &str) -> Result<String, RollbackError>;
    fn head(&self, repo: &Path) -> Result<String, RollbackError>;
}
