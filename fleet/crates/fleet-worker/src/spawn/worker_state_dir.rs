//! Where `join` records the per-agent scorecard. This is fleet's OWN bookkeeping about workers,
//! not the user's repo's data -- it must never land inside `handle.repo` (a real `.fleet/state`
//! there used to show up as `?? .fleet/` in the user's `git status` after every single `swarm`
//! run). Honors `FLEET_STATE_DIR`, the same env var name `runtime::config::Config::state_dir`
//! reads on the CLI side (`Env::prefixed("FLEET_")`), so one override controls both; unset, this
//! crate defaults to a per-user XDG-style state dir, never a path under `repo`.

use std::path::PathBuf;

const ENV_STATE_DIR: &str = "FLEET_STATE_DIR";

/// Resolve fleet-worker's global state root (parent of `scorecards/`). Never returns a path
/// inside any target repo.
pub fn resolve() -> PathBuf {
    if let Ok(dir) = std::env::var(ENV_STATE_DIR) {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| std::env::temp_dir().display().to_string());
    PathBuf::from(home).join(".local").join("state").join("fleet")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Both cases live in one test (not two) -- `std::env::set_var`/`remove_var` are process-wide,
    // and `cargo test` runs tests in the same binary concurrently by default, so two tests each
    // mutating `ENV_STATE_DIR` would race each other.
    #[test]
    fn honors_env_override_and_falls_back_to_an_absolute_non_repo_default() {
        std::env::set_var(ENV_STATE_DIR, "/tmp/fleet-state-override-test");
        assert_eq!(resolve(), PathBuf::from("/tmp/fleet-state-override-test"));

        std::env::remove_var(ENV_STATE_DIR);
        let resolved = resolve();
        assert!(resolved.is_absolute(), "default state dir must be absolute: {resolved:?}");
        assert!(!resolved.ends_with(".fleet"), "must not reuse the repo-local .fleet/ shape");
    }
}
