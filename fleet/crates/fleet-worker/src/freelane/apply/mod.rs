//! Turns a freelane reply into applied changes. The predecessor (`git show
//! HEAD:fleet/keel/fleet/src/main.rs:3420`) extracted fenced code and appended it to a single
//! hardcoded file (`main.rs`) regardless of what the reply was actually about. That is exactly
//! the failure mode this module refuses to repeat: extract every fenced block, require EACH one
//! to name its own target unambiguously, validate ALL targets before writing ANY (one bad file
//! refuses the whole batch -- see `write::write_all`), then write. No target is ever guessed.
//!
//! Reply format supported: a fenced code block whose target is declared either in the fence's
//! own info string (` ```path:src/foo.rs `) or on a `path:`/`file:` line immediately above it
//! (see `parse` for the exact grammar). Chosen because it is trivial for a model to produce
//! on request, trivial to parse without a Markdown library, and unambiguous -- no heuristic
//! guessing a path from a bare language tag.

mod error;
mod guard;
mod parse;
mod write;

pub use error::ApplyError;

use parse::extract_fences;
use std::path::{Path, PathBuf};

/// Applies every fenced block in `reply` to `worktree`, or refuses the whole reply. Never writes
/// a partial batch: every target is resolved and containment-checked before the first write.
pub fn apply(worktree: &Path, reply: &str) -> Result<Vec<PathBuf>, ApplyError> {
    let fences = extract_fences(reply);
    if fences.is_empty() {
        return Err(ApplyError::NoFence);
    }
    let mut resolved: Vec<(PathBuf, String)> = Vec::with_capacity(fences.len());
    for fence in &fences {
        let raw = fence.declared_path.as_deref().ok_or(ApplyError::AmbiguousTarget { index: fence.fence_index })?;
        let target = guard::resolve_target(worktree, fence.fence_index, raw)?;
        resolved.push((target, fence.content.clone()));
    }
    write::write_all(&resolved)
}

/// `run`'s call site: never an error path for `run` itself -- see `FreelaneOutput::apply_note`.
/// The downstream honesty check (`fleet_worker::spawn::change_detect`) is what turns an
/// unapplied reply into a refusal, by looking at the real worktree diff, not at this return.
pub fn try_apply(worktree: &Path, response: &str) -> (Vec<PathBuf>, Option<String>) {
    match apply(worktree, response) {
        Ok(paths) => (paths, None),
        Err(err) => (Vec::new(), Some(err.to_string())),
    }
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
