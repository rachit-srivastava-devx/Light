//! Merge-back gate -- verbatim port of `merge-lane.sh`'s control flow, each refusal line
//! replaced by a typed `MergeRefusal`, each invariant delegated to `invariant`'s pure checks at
//! the point the corresponding shell guard runs today.

use crate::error::{MergeRefusal, PrError};
use crate::git_exec::{count_lines, run_git};
use crate::invariant::{check_files_changed, check_head_moved, check_stage_nonempty};
use std::path::Path;
use std::process::Command;

/// What a successful, verified merge actually moved -- measured after the fact from real git
/// output, never assumed from a `git merge` exit code alone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergeOutcome {
    pub branch: String,
    /// Files staged inside the worktree before the lane's own commit (`merge-lane.sh:11`).
    pub staged_files: usize,
    /// Files differing between pre-/post-merge `HEAD` in the target repo (`merge-lane.sh:21`).
    pub changed_files: usize,
    /// Target repo's `HEAD` sha before the merge (`merge-lane.sh:17`).
    pub before: String,
    /// Target repo's `HEAD` sha after the merge (`merge-lane.sh:19`).
    pub after: String,
}

/// Stage, commit, and merge one lane's worktree branch into the target repo's `HEAD`, enforcing
/// all three D31 invariants in order, exactly as `merge-lane.sh:10-23` does today.
pub fn merge_lane(
    repo: &Path,
    worktree_dir: &Path,
    branch: &str,
) -> Result<MergeOutcome, MergeRefusal> {
    if !worktree_dir.is_dir() {
        return Err(MergeRefusal::NoWorktree(worktree_dir.to_path_buf()));
    }
    let add = run_git(worktree_dir, &["add", "-A", ":!*.pyc", ":!*/target/*"])
        .map_err(MergeRefusal::Spawn)?;
    if !add.status.success() {
        return Err(MergeRefusal::StageFailed(worktree_dir.to_path_buf()));
    }
    let staged_out =
        run_git(worktree_dir, &["diff", "--cached", "--name-only"]).map_err(MergeRefusal::Spawn)?;
    let staged_files = count_lines(&staged_out.stdout);
    check_stage_nonempty(staged_files, branch)?;

    let msg = format!("lane {branch}: {staged_files} files");
    let commit = run_git(
        worktree_dir,
        &["-c", "core.hooksPath=/dev/null", "commit", "-qm", &msg],
    )
    .map_err(MergeRefusal::Spawn)?;
    if !commit.status.success() {
        return Err(MergeRefusal::CommitFailed {
            branch: branch.to_string(),
        });
    }

    let before = rev_parse_head(repo)?;
    let merge = run_git(
        repo,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "merge",
            "--no-edit",
            "-q",
            branch,
        ],
    )
    .map_err(MergeRefusal::Spawn)?;
    if !merge.status.success() {
        return Err(MergeRefusal::Conflict {
            branch: branch.to_string(),
        });
    }
    let after = rev_parse_head(repo)?;
    check_head_moved(&before, &after, branch)?;

    let diff_out =
        run_git(repo, &["diff", "--name-only", &before, &after]).map_err(MergeRefusal::Spawn)?;
    let changed_files = count_lines(&diff_out.stdout);
    check_files_changed(changed_files, branch)?;

    Ok(MergeOutcome {
        branch: branch.to_string(),
        staged_files,
        changed_files,
        before,
        after,
    })
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
    // For now, we'll construct a basic title and body
    // Parse module_brief JSON to extract module_id and description if possible
    // For now, use branch as module_id placeholder
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

fn rev_parse_head(repo: &Path) -> Result<String, MergeRefusal> {
    let out = run_git(repo, &["rev-parse", "HEAD"]).map_err(MergeRefusal::Spawn)?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Result of a successful pr_emit operation.
#[derive(Debug, Clone)]
pub struct PrOutcome {
    pub pr_url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pr_emit_github_cli_check() {
        // This test just verifies gh CLI availability
        let available = Command::new("gh")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        println!("gh CLI available: {}", available);
    }
}
