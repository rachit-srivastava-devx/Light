//! Merge-back gate -- verbatim port of `merge-lane.sh`'s control flow, each refusal line
//! replaced by a typed `MergeRefusal`, each invariant delegated to `invariant`'s pure checks at
//! the point the corresponding shell guard runs today.

use crate::error::MergeRefusal;
use crate::git_exec::{count_lines, run_git};
use crate::invariant::{check_files_changed, check_head_moved, check_stage_nonempty};
use std::path::Path;

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
    let staged_out = run_git(worktree_dir, &["diff", "--cached", "--name-only"])
        .map_err(MergeRefusal::Spawn)?;
    let staged_files = count_lines(&staged_out.stdout);
    check_stage_nonempty(staged_files, branch)?;

    let msg = format!("lane {branch}: {staged_files} files");
    let commit = run_git(
        worktree_dir,
        &["-c", "core.hooksPath=/dev/null", "commit", "-qm", &msg],
    )
    .map_err(MergeRefusal::Spawn)?;
    if !commit.status.success() {
        return Err(MergeRefusal::CommitFailed { branch: branch.to_string() });
    }

    let before = rev_parse_head(repo)?;
    let merge = run_git(
        repo,
        &["-c", "core.hooksPath=/dev/null", "merge", "--no-edit", "-q", branch],
    )
    .map_err(MergeRefusal::Spawn)?;
    if !merge.status.success() {
        return Err(MergeRefusal::Conflict { branch: branch.to_string() });
    }
    let after = rev_parse_head(repo)?;
    check_head_moved(&before, &after, branch)?;

    let diff_out =
        run_git(repo, &["diff", "--name-only", &before, &after]).map_err(MergeRefusal::Spawn)?;
    let changed_files = count_lines(&diff_out.stdout);
    check_files_changed(changed_files, branch)?;

    Ok(MergeOutcome { branch: branch.to_string(), staged_files, changed_files, before, after })
}

fn rev_parse_head(repo: &Path) -> Result<String, MergeRefusal> {
    let out = run_git(repo, &["rev-parse", "HEAD"]).map_err(MergeRefusal::Spawn)?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
