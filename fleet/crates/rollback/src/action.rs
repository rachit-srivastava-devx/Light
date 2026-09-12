use crate::{GitRollbackPort, RollbackError, RollbackReceipt};
use std::path::Path;

pub(crate) fn apply_private(
    git: &impl GitRollbackPort,
    artifact_id: &str,
    path: &Path,
) -> Result<RollbackReceipt, RollbackError> {
    let before = path.display().to_string();
    git.remove_owned(path)?;
    let after = crate::reconcile::verify_removal(path)?;
    Ok(crate::receipt::success(artifact_id, &before, &after))
}

pub(crate) fn apply_shared(
    _git: &impl GitRollbackPort,
    artifact_id: &str,
    repo: &Path,
    merge_commit: &str,
) -> Result<RollbackReceipt, RollbackError> {
    // Shared refs require a human-approved compensating revert; reset --hard is forbidden.
    Err(RollbackError::NeedsApproval(format!(
        "artifact={artifact_id} repo={} commit={merge_commit}: create a git revert proposal",
        repo.display()
    )))
}
