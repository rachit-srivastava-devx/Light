//! The per-user default for `Config::state_dir`, used only when `FLEET_STATE_DIR` (and any
//! future config-file layer) leaves it unset. Split out of `config.rs` for the 80-line cap.
//!
//! Same shape `fleet-worker::spawn::worker_state_dir::resolve` falls back to, so the CLI and the
//! worker agree on where state lives when neither side is given an explicit override. Never a
//! path under the CWD -- that was the defect (see `config.rs`'s module doc comment).

use crate::runtime::config_error::ConfigError;
use std::path::PathBuf;

/// Env var name kept in sync with `fleet-worker::spawn::worker_state_dir`'s `ENV_STATE_DIR`.
pub const ENV_STATE_DIR: &str = "FLEET_STATE_DIR";

/// `$XDG_STATE_HOME/fleet` if set (platform convention), else `$HOME/.local/state/fleet`.
pub fn default_state_dir() -> Result<PathBuf, ConfigError> {
    if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
        if !xdg.trim().is_empty() {
            return Ok(PathBuf::from(xdg).join("fleet"));
        }
    }
    let home = std::env::var("HOME").map_err(|_| ConfigError::HomeUnset)?;
    Ok(PathBuf::from(home).join(".local").join("state").join("fleet"))
}

/// One-shot notice (this runs once per process -- `load` calls it at most once, and `main` calls
/// `load` exactly once) so a user with pre-existing CWD-relative state understands why
/// `fleet ledger` etc. suddenly look empty, without fleet silently orphaning or auto-migrating
/// that directory.
pub fn warn_if_cwd_state_orphaned(new_default: &std::path::Path) {
    let cwd_state = std::path::Path::new(".fleet-state");
    if !cwd_state.is_dir() {
        return;
    }
    let Ok(cwd) = std::env::current_dir() else { return };
    let old = cwd.join(cwd_state);
    eprintln!(
        "fleet: found existing state at {} but the default state dir is now {} \
         (set {ENV_STATE_DIR}={} to keep using the old location)",
        old.display(),
        new_default.display(),
        old.display(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_dir_is_absolute_and_not_the_repo_local_shape() {
        let dir = default_state_dir();
        // `$HOME`/`$XDG_STATE_HOME` are environment-dependent in CI; only assert the shape.
        if let Ok(dir) = dir {
            assert!(dir.is_absolute(), "default state dir must be absolute: {dir:?}");
            assert!(dir.ends_with("fleet"), "default state dir must end in fleet/: {dir:?}");
        }
    }
}
