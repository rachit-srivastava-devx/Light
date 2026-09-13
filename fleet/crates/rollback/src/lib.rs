//! STATUS: the grant/reason/before fields on this crate's types are unwired scaffolding
//! matching `docs/LLD/LLD.md` §14 — not dead code, no caller yet.
mod action;
mod error;
mod guard;
mod port;
mod receipt;
mod reconcile;
mod types;

pub use error::RollbackError;
pub use port::{GitRollbackPort, RollbackStore};
pub use reconcile::verify_action;
pub use types::{ArtifactRecord, Grant, RollbackReceipt, RollbackRequest, RollbackTarget};

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
    if let RollbackTarget::PrivateRef {
        ref repo,
        ref applied,
        ..
    } = req.target
    {
        if let Err(e) = guard::check_cas(git, repo, applied) {
            store.write_receipt(&receipt::refuse(&req.artifact_id));
            return Err(e);
        }
    }
    let result = match req.target {
        RollbackTarget::OwnedWorktree(ref p) => action::apply_private(git, &req.artifact_id, p),
        RollbackTarget::PrivateRef { ref repo, .. } => {
            action::apply_private(git, &req.artifact_id, repo)
        }
        RollbackTarget::SharedRef {
            ref repo,
            ref merge_commit,
        } => action::apply_shared(git, &req.artifact_id, repo, merge_commit),
    };
    match result {
        Ok(r) => {
            store.write_receipt(&r);
            Ok(r)
        }
        Err(e) => {
            store.write_receipt(&receipt::refuse(&req.artifact_id));
            Err(e)
        }
    }
}
