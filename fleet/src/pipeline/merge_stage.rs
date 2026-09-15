//! `Merge`'s wiring, split out of `stages.rs` to stay under the 80-line file gate. Derives the
//! staged-file count and branch from the real repo the CLI was pointed at (`--repo`) instead of
//! the hardcoded `(1, "pipeline-demo-branch")` this stage used to pass `integrate` unconditionally.
//! No git work tree at that path is reported through `integrate::MergeRefusal::NoWorktree` --
//! the same typed refusal `integrate` already uses for "nothing to merge" -- rather than
//! synthesizing a fake success.

use super::event::PipelineError;
use integrate::MergeRefusal;
use print::human_stream::emit;
use print::render_event::Event;
use print::style::Style;
use std::path::Path;
use std::process::Command;

pub fn merge(repo: &Path) -> Result<(), PipelineError> {
    if !is_work_tree(repo) {
        return Err(PipelineError::Merge(MergeRefusal::NoWorktree(
            repo.to_path_buf(),
        )));
    }
    let staged = git(repo, &["diff", "--cached", "--name-only"])?;
    let staged_files = staged.lines().filter(|l| !l.trim().is_empty()).count();
    let branch = git(repo, &["symbolic-ref", "--short", "HEAD"])?;
    let branch = branch.trim();
    emit(
        &Event::Note {
            source: "merge".into(),
            text: format!("branch={branch} staged_files={staged_files} (guard only, no commit)"),
        },
        &Style::detect(),
    );
    integrate::check_stage_nonempty(staged_files, branch).map_err(PipelineError::Merge)
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
