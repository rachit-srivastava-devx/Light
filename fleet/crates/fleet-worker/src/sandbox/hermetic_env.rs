//! The single most load-bearing divergence from a straight port (BLUEPRINT.md divergence #6):
//! today's `spawn_agent_with_args` PASSES THROUGH the parent's real `HOME`. This module instead
//! computes a fresh per-lane tempdir and points `HOME`/`XDG_*` there, so a spawned CLI never
//! sees the user's real `~/.claude`/`~/.codex`.

use std::env;
use std::path::{Path, PathBuf};

/// The env vars a hermetically-spawned child process is allowed to see. Built by `env_clear()`
/// + this exact allowlist -- nothing else crosses the boundary.
#[derive(Clone, Debug)]
pub struct HermeticEnv {
    pub home: PathBuf,
    pub xdg_config_home: PathBuf,
    pub xdg_data_home: PathBuf,
    pub xdg_cache_home: PathBuf,
    pub path: String,
    pub lang: Option<String>,
    pub cargo_target_dir: Option<String>,
}

/// Allocate a fresh per-lane tempdir (never the user's real `$HOME`) and build the allowlisted
/// env this lane's child process will see. The caller owns removing `root` after `join`.
pub fn build(root: &Path) -> HermeticEnv {
    let home = root.join("home");
    let xdg_config_home = home.join(".config");
    let xdg_data_home = home.join(".local").join("share");
    let xdg_cache_home = home.join(".cache");
    for dir in [&home, &xdg_config_home, &xdg_data_home, &xdg_cache_home] {
        let _ = std::fs::create_dir_all(dir);
    }
    HermeticEnv {
        home,
        xdg_config_home,
        xdg_data_home,
        xdg_cache_home,
        path: env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string()),
        lang: env::var("LANG").ok(),
        // Shared build cache, not a credential -- passed through only when the parent set it
        // (never hardcoded), same reasoning as `main.rs:3060-3066`.
        cargo_target_dir: env::var("CARGO_TARGET_DIR").ok(),
    }
}

impl HermeticEnv {
    /// Apply this env to `command` via `env_clear()` + explicit `.env(...)` calls -- the child
    /// process inherits NOTHING else, in particular never the real `HOME`/`XDG_*`.
    pub fn apply(&self, command: &mut std::process::Command) {
        command.env_clear();
        command.env("PATH", &self.path);
        command.env("HOME", &self.home);
        command.env("XDG_CONFIG_HOME", &self.xdg_config_home);
        command.env("XDG_DATA_HOME", &self.xdg_data_home);
        command.env("XDG_CACHE_HOME", &self.xdg_cache_home);
        if let Some(lang) = &self.lang {
            command.env("LANG", lang);
        }
        if let Some(dir) = &self.cargo_target_dir {
            command.env("CARGO_TARGET_DIR", dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_never_reuses_the_real_home() {
        let dir = tempfile::tempdir().unwrap();
        let env = build(dir.path());
        assert!(env.home.starts_with(dir.path()));
        assert_ne!(env.home, PathBuf::from(env::var("HOME").unwrap_or_default()));
    }
}
