//! Best-effort build-cache invalidation -- `fleet/bin/merge-lane.sh:25-33` (D34/M3), verbatim.

use std::path::Path;
use std::process::{Command, Stdio};

/// After a successful merge, invalidate the cached build artifact so a stale
/// `CARGO_MANIFEST_DIR`/`CARGO_TARGET_DIR` baked in at compile time from a now-removed worktree
/// never silently serves a green build pointing at a path that no longer exists. Deliberately
/// returns `bool` (ran vs. did-not-run), never `Result` -- a failure inside `cargo clean` is
/// swallowed exactly as `merge-lane.sh:31`'s `|| true` swallows it: this step is advisory cache
/// hygiene, never allowed to turn a verified merge into a refusal. Only runs if `cargo` is on
/// `PATH` and `manifest_path` exists, matching `merge-lane.sh:30`'s guard.
pub fn invalidate_build_cache(manifest_path: &Path, package: &str) -> bool {
    if !manifest_path.exists() {
        return false;
    }
    let cargo_on_path = Command::new("cargo")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !cargo_on_path {
        return false;
    }
    let _ = Command::new("cargo")
        .args(["clean", "-q", "-p", package, "--manifest-path"])
        .arg(manifest_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    true
}
