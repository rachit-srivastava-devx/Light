//! `Merge`'s wiring, split out of `stages.rs` to stay under the 80-line file gate. Derives the
//! staged-file count and branch from the real repo the CLI was pointed at (`--repo`) instead of
//! the hardcoded `(1, "pipeline-demo-branch")` this stage used to pass `fleet_merge` unconditionally.
//! No git work tree at that path is reported through `fleet_merge::MergeRefusal::NoWorktree` --
//! the same typed refusal `fleet_merge` already uses for "nothing to merge" -- rather than
//! synthesizing a fake success.

use super::event::PipelineError;
use fleet_merge::MergeRefusal;
use std::path::Path;
use std::process::Command;

pub fn merge(repo: &Path) -> Result<(), PipelineError> {
    if !is_work_tree(repo) {
        return Err(PipelineError::Merge(MergeRefusal::NoWorktree(repo.to_path_buf())));
    }
    let staged = git(repo, &["diff", "--cached", "--name-only"])?;
    let staged_files = staged.lines().filter(|l| !l.trim().is_empty()).count();
    let branch = git(repo, &["symbolic-ref", "--short", "HEAD"])?;
    fleet_merge::check_stage_nonempty(staged_files, branch.trim()).map_err(PipelineError::Merge)
}

/// `git rev-parse --is-inside-work-tree` succeeds for ANY path inside a real git checkout
/// (unlike checking for a `.git` entry directly under `repo`, which only matches the exact
/// repo root, not `--repo`s pointed at a subdirectory of a real work tree).
fn is_work_tree(repo: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git(repo: &Path, args: &[&str]) -> Result<String, PipelineError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| PipelineError::Runtime(format!("git spawn failed: {e}")))?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
