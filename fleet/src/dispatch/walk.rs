//! Bounded, skip-list-aware source-file walk for `fleet graph|impact` -- the actual filesystem IO
//! `fleet-context` deliberately does not do itself (see its `lib.rs` doc comment: "no filesystem
//! walk"). Skips known build/VCS/vendor directories and enforces a file-count/byte/wall-clock
//! budget so a real ~450MB repo (`target/`, `.git/`, `.worktrees/`, `node_modules/`, `.venv/`)
//! finishes in bounded time instead of spinning forever walking generated or vendored trees.

use fleet_context::{language_for, SourceFile};
use std::path::Path;
use std::time::{Duration, Instant};

pub use crate::dispatch::walk_error::WalkError;

const SKIP_DIRS: &[&str] =
    &["target", ".git", ".worktrees", "node_modules", ".venv", "__pycache__", "dist", "build"];
const MAX_FILES: usize = 50_000;
const MAX_BYTES: u64 = 500_000_000;
const DEADLINE: Duration = Duration::from_secs(60);

struct Budget {
    started: Instant,
    bytes: u64,
}

/// Checked once, up front, for the root only -- a nonexistent/unreadable `--repo` must be a
/// clear error, never a silent fallback to the process's cwd (see `WalkError::RepoUnreadable`).
/// Shared by every `--repo`-taking command (`graph`/`impact` via `read_source_files_bounded`
/// below, `gate`/`oracle` via `verify_cmd.rs`) so "refuse an unusable target" is one rule, not
/// one per caller.
pub fn ensure_repo_readable(repo: &Path) -> Result<(), WalkError> {
    std::fs::read_dir(repo).map(|_| ()).map_err(|e| WalkError::RepoUnreadable(repo.to_path_buf(), e.to_string()))
}

/// Walks `repo` for `fleet-context`-parseable source files, skipping `SKIP_DIRS` and refusing
/// (with a typed `WalkError`) rather than hanging once any of the file/byte/deadline budgets is
/// exceeded.
pub fn read_source_files_bounded(repo: &Path) -> Result<Vec<SourceFile>, WalkError> {
    // A subdirectory becoming unreadable mid-walk still just gets skipped, unchanged from before
    // -- only the root the caller explicitly named must be a hard error.
    ensure_repo_readable(repo)?;
    let mut out = Vec::new();
    let mut budget = Budget { started: Instant::now(), bytes: 0 };
    collect(repo, repo, &mut out, &mut budget)?;
    Ok(out)
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<SourceFile>, budget: &mut Budget) -> Result<(), WalkError> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Ok(()) };
    for entry in entries.flatten() {
        if budget.started.elapsed() > DEADLINE {
            return Err(WalkError::DeadlineExceeded(DEADLINE, dir.to_path_buf()));
        }
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !SKIP_DIRS.contains(&name) {
                collect(root, &path, out, budget)?;
            }
        } else if let Some(language) = language_for(&path) {
            if out.len() >= MAX_FILES {
                return Err(WalkError::TooManyFiles(MAX_FILES, path));
            }
            if let Ok(source) = std::fs::read_to_string(&path) {
                budget.bytes += source.len() as u64;
                if budget.bytes > MAX_BYTES {
                    return Err(WalkError::TooManyBytes(MAX_BYTES, path));
                }
                let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                out.push(SourceFile { path: rel, language, source });
            }
        }
    }
    Ok(())
}
