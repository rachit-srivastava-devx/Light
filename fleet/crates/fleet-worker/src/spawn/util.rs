//! Small shared helpers for the `spawn` module.

use std::path::PathBuf;

/// Test-only seam: when set, this crate execs the named binary instead of
/// `env::current_exe()` for the child side. Production code never sets this env var; it exists
/// solely so integration tests can stand a fixture binary in for the real `src/` `__agent`
/// dispatch (BLUEPRINT.md §10 flags that no such seam exists in the §3 contract -- this is the
/// narrowest one that doesn't change any public signature).
pub const CHILD_EXE_OVERRIDE: &str = "FLEET_WORKER_TEST_CHILD_EXE";

/// Ported from `main.rs::which_on_path`.
pub fn which_on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
}
