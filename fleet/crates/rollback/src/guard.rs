use crate::{ArtifactRecord, GitRollbackPort, RollbackError, RollbackStore};
use std::path::{Component, Path, PathBuf};

pub(crate) fn check_artifact(
    store: &impl RollbackStore,
    id: &str,
) -> Result<ArtifactRecord, RollbackError> {
    match store.get_artifact(id) {
        Some(rec) if rec.status == "rolled_back" => {
            Err(RollbackError::Receipt(format!("{id} already rolled back")))
        }
        Some(rec) => Ok(rec),
        None => Err(RollbackError::ArtifactNotFound(id.to_string())),
    }
}

pub(crate) fn check_containment(root: &Path, target: &Path) -> Result<(), RollbackError> {
    let canon_root = root.canonicalize().unwrap_or_else(|_| normalize(root));
    let canon_target = target.canonicalize().unwrap_or_else(|_| normalize(target));
    if canon_target.starts_with(&canon_root) {
        Ok(())
    } else {
        Err(RollbackError::Containment(format!(
            "{} is outside worktrees root {}",
            target.display(),
            root.display()
        )))
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

pub(crate) fn check_cas(
    git: &impl GitRollbackPort,
    repo: &Path,
    applied: &str,
) -> Result<(), RollbackError> {
    let head = git.head(repo)?;
    if head != applied {
        Err(RollbackError::CasMismatch(format!(
            "HEAD is {head} but applied is {applied}"
        )))
    } else {
        Ok(())
    }
}
