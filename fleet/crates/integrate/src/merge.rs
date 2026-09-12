use crate::{IntegrateError, MergeRequest};
use std::path::Path;
use std::process::Command;

/// Port for git merge operations. Injected for testability.
pub trait GitPort {
    /// Pre-flight conflict check using `git merge-tree`. No repo mutation.
    fn merge_tree(&self, req: &MergeRequest) -> Result<(), IntegrateError>;
    /// Perform `git merge --no-ff`. Returns the new HEAD sha on success.
    fn merge(&self, req: &MergeRequest) -> Result<String, IntegrateError>;
}

/// Production implementation that shells out to the host `git` binary.
pub struct RealGit;

fn git_cmd(
    repo: &Path,
    args: &[&str],
) -> Result<std::process::Output, IntegrateError> {
    Command::new("git")
        .args(args)
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", "fleet-integrate")
        .env("GIT_AUTHOR_EMAIL", "fleet@localhost")
        .env("GIT_COMMITTER_NAME", "fleet-integrate")
        .env("GIT_COMMITTER_EMAIL", "fleet@localhost")
        .output()
        .map_err(|e| IntegrateError::Git { msg: e.to_string() })
}

fn rev_parse_head(repo: &Path) -> Result<String, IntegrateError> {
    let out = git_cmd(repo, &["rev-parse", "HEAD"])?;
    if !out.status.success() {
        return Err(IntegrateError::Git { msg: "rev-parse HEAD failed".into() });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

impl GitPort for RealGit {
    fn merge_tree(&self, req: &MergeRequest) -> Result<(), IntegrateError> {
        let out = Command::new("git")
            .args(["merge-tree", "--write-tree", "HEAD", &req.lane_head])
            .current_dir(&req.repo)
            .output()
            .map_err(|e| IntegrateError::Git { msg: e.to_string() })?;
        if !out.status.success() {
            // Distinguish an unsupported flag (old git) from a real conflict.
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("unknown option") || stderr.contains("usage:") {
                return Ok(()); // old git: let merge() handle conflict detection
            }
            return Err(IntegrateError::Conflict { branch: req.lane_head.clone() });
        }
        Ok(())
    }

    fn merge(&self, req: &MergeRequest) -> Result<String, IntegrateError> {
        let out = git_cmd(
            &req.repo,
            &["-c", "core.hooksPath=/dev/null", "merge", "--no-ff", "-q", &req.lane_head],
        )?;
        if !out.status.success() {
            let _ = git_cmd(&req.repo, &["merge", "--abort"]);
            return Err(IntegrateError::Conflict { branch: req.lane_head.clone() });
        }
        rev_parse_head(&req.repo)
    }
}
