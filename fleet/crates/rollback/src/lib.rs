mod action; mod guard; mod receipt; mod reconcile;

use std::path::{Path, PathBuf};

pub use reconcile::verify_action;

pub enum RollbackTarget {
    OwnedWorktree(PathBuf),
    PrivateRef { repo: PathBuf, before: String, applied: String },
    SharedRef { repo: PathBuf, merge_commit: String },
}

pub struct Grant { pub allows: Vec<String> }
pub struct ArtifactRecord { pub status: String }
pub struct RollbackRequest {
    pub artifact_id: String, pub target: RollbackTarget, pub reason: String, pub grant: Grant,
}
pub struct RollbackReceipt {
    pub artifact_id: String, pub status: String, pub before: String, pub after: String,
    pub checked: u64, pub total: u64,
}

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

#[derive(Debug, thiserror::Error)]
pub enum RollbackError {
    #[error("artifact not found: {0}")] ArtifactNotFound(String),
    #[error("containment: {0}")] Containment(String),
    #[error("CAS mismatch: {0}")] CasMismatch(String),
    #[error("needs approval: {0}")] NeedsApproval(String),
    #[error("receipt error: {0}")] Receipt(String),
    #[error("needs reconciliation: {0}")] NeedsReconciliation(String),
}

pub fn rollback(
    git: &impl GitRollbackPort,
    store: &mut impl RollbackStore,
    req: RollbackRequest,
) -> Result<RollbackReceipt, RollbackError> {
    let root = store.worktrees_root().to_path_buf();
    let guard_result = guard::check_artifact(store, &req.artifact_id);
    if let Err(e) = guard_result {
        store.write_receipt(&receipt::refuse(&req.artifact_id));
        return Err(e);
    }
    if let RollbackTarget::OwnedWorktree(ref p) = req.target {
        if let Err(e) = guard::check_containment(&root, p) {
            store.write_receipt(&receipt::refuse(&req.artifact_id));
            return Err(e);
        }
    }
    if let RollbackTarget::PrivateRef { ref repo, ref applied, .. } = req.target {
        if let Err(e) = guard::check_cas(git, repo, applied) {
            store.write_receipt(&receipt::refuse(&req.artifact_id));
            return Err(e);
        }
    }
    let result = match req.target {
        RollbackTarget::OwnedWorktree(ref p) => action::apply_private(git, &req.artifact_id, p),
        RollbackTarget::PrivateRef { ref repo, .. } => action::apply_private(git, &req.artifact_id, repo),
        RollbackTarget::SharedRef { ref repo, ref merge_commit } => {
            action::apply_shared(git, &req.artifact_id, repo, merge_commit)
        }
    };
    match result {
        Ok(r) => { store.write_receipt(&r); Ok(r) }
        Err(e) => { store.write_receipt(&receipt::refuse(&req.artifact_id)); Err(e) }
    }
}
