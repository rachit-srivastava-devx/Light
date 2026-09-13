//! `gh pr create` from a merged lane branch. Separate from `merge.rs`'s own job (staging,
//! committing, and merging a lane's worktree back) -- this is the external-publication step
//! after that has already succeeded.

use crate::error::PrError;
use crate::git_exec::run_git;
use std::path::Path;
use std::process::Command;

/// Result of a successful `pr_emit` operation.
#[derive(Debug, Clone)]
pub struct PrOutcome {
    pub pr_url: String,
}

/// Create a GitHub pull request from branch work using `gh pr create`.
/// Returns the PR URL and metadata.
pub fn pr_emit(
    repo: &Path,
    _worktree_dir: &Path,
    branch: &str,
    module_brief: &str,
    diff_summary: &str,
) -> Result<PrOutcome, PrError> {
    // Check that gh CLI is available
    if !Command::new("gh")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Err(PrError::GhCliNotFound);
    }

    // Get the current HEAD branch as base
    let head = run_git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map_err(|e| PrError::CreateFailed(e.to_string()))?;
    let base = String::from_utf8_lossy(&head.stdout).trim().to_string();

    // Create PR with title and body generated from module_brief and diff_summary
    let title = format!("Module: {} - Changes from lane", branch);

    let body = format!(
        "## Module Brief\n{}\n\n## Diff Summary\n{}",
        module_brief, diff_summary
    );

    // Run gh pr create with title, body, base, and head
    let pr = Command::new("gh")
        .arg("pr")
        .arg("create")
        .arg("--title")
        .arg(&title)
        .arg("--body")
        .arg(&body)
        .arg("--base")
        .arg(&base)
        .arg("--head")
        .arg(branch)
        .current_dir(repo)
        .output()
        .map_err(|e| PrError::CreateFailed(e.to_string()))?;

    if !pr.status.success() {
        return Err(PrError::CreateFailed(
            String::from_utf8_lossy(&pr.stderr).to_string(),
        ));
    }

    // Parse the PR URL from stdout (format: "https://github.com/owner/repo/pull/123")
    let pr_url = String::from_utf8_lossy(&pr.stdout).trim().to_string();

    Ok(PrOutcome { pr_url })
}
