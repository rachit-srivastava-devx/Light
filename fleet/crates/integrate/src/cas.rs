use crate::{IntegrateError, MergeRequest};
use std::path::Path;
use std::process::Command;

/// Read the actual HEAD of `req.repo` and compare with `req.expected_head`.
/// Returns the actual HEAD on success (to use as `before` in the receipt).
pub fn cas_predicate(req: &MergeRequest) -> Result<String, IntegrateError> {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&req.repo)
        .output()
        .map_err(|e| IntegrateError::Git { msg: e.to_string() })?;
    if !out.status.success() {
        return Err(IntegrateError::Git { msg: "rev-parse HEAD failed".into() });
    }
    let actual = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if actual != req.expected_head {
        return Err(IntegrateError::CasMismatch {
            expected: req.expected_head.clone(),
            actual,
        });
    }
    Ok(actual)
}

/// Verify that `candidate` resolves to a path under `<repo>/.worktrees/`.
pub fn path_containment_check(repo: &Path, candidate: &Path) -> Result<(), IntegrateError> {
    let worktrees_root = repo.join(".worktrees");
    let canonical_root = worktrees_root
        .canonicalize()
        .unwrap_or_else(|_| worktrees_root.clone());
    let canonical_candidate = candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.to_path_buf());
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(IntegrateError::Containment {
            path: candidate.to_string_lossy().into_owned(),
        });
    }
    Ok(())
}

/// Validate the grant in `req`: expiry, target ref, and candidate digest.
pub fn check_grant(req: &MergeRequest) -> Result<(), IntegrateError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if req.grant.expires_at <= now {
        return Err(IntegrateError::Grant { msg: "grant expired".into() });
    }
    if req.grant.target_ref != req.lane_head {
        return Err(IntegrateError::Grant { msg: "target ref mismatch".into() });
    }
    if req.grant.candidate_digest != req.candidate_digest {
        return Err(IntegrateError::Grant { msg: "digest mismatch".into() });
    }
    if req.checked == 0 || req.checked != req.total {
        return Err(IntegrateError::Grant { msg: "checked/total invariant violated".into() });
    }
    Ok(())
}
