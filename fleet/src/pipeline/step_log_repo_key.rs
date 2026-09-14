//! `repo_key`: a stable per-repo suffix for `StepLog::open_for_repo` -- split out of
//! `step_log.rs` to stay under the 80-line file cap.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

/// Derived from the repo's canonicalized path (falls back to the as-given path if
/// canonicalization fails -- still deterministic per invocation, just not collision-proof
/// across a rename). `fleet run --repo a --repo b --task X` shares one `state_dir` across both
/// repos (per-user by default, never per-repo -- see `runtime::state_dir_default`), so without
/// this repo-scoped key the two repos would open the SAME step log file for the same task id:
/// repo B would see every stage already marked done by repo A's run and report them all as
/// "resumed" without actually executing anything against repo B.
pub fn repo_key(repo: &Path) -> String {
    let canonical = std::fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf());
    let mut h = DefaultHasher::new();
    canonical.hash(&mut h);
    format!("{:016x}", h.finish())
}
