//! Layered config: env vars over defaults, one typed `Config` struct, replacing the ad hoc
//! `env::var("FLEET_...")` reads scattered through the old `main.rs`/`route.rs`/`meter.rs`.
//! Deviation from BLUEPRINT §7: only the `env` figment provider is wired (no file layer yet) --
//! flagged, not silently dropped; add a `Toml::file(...)` provider when a config file format is
//! decided.
//!
//! `state_dir`'s default used to be the CWD-relative `.fleet-state` -- fine in a scratch dir, but
//! the CWD is normally the user's own repo, so fleet wrote an untracked `.fleet-state/` (ledger,
//! meter, step logs) straight into their checkout (same defect class as the `.fleet/` scorecard
//! fixed in `fleet-worker::spawn::worker_state_dir`, just via a different path). The default now
//! matches that module: a per-user XDG-style state dir (`state_dir_default.rs`), never a path
//! under the CWD. `FLEET_STATE_DIR` remains the explicit override and still wins.

use crate::runtime::config_error::ConfigError;
use crate::runtime::state_dir_default::{default_state_dir, warn_if_cwd_state_orphaned};
use figment::providers::Env;
use figment::Figment;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Config {
    pub state_dir: PathBuf,
    pub ram_lanes: Option<usize>,
    pub review_cap: usize,
}

/// Mirrors `Config` but leaves `state_dir` unset when `FLEET_STATE_DIR` isn't in the
/// environment, so `load` can tell "explicit override" apart from "needs a computed default"
/// instead of baking a CWD-relative default into the `serde(default)` path.
#[derive(Debug, Deserialize)]
struct RawConfig {
    state_dir: Option<String>,
    #[serde(default)]
    ram_lanes: Option<usize>,
    #[serde(default = "default_review_cap")]
    review_cap: usize,
}

fn default_review_cap() -> usize {
    3
}

pub fn load() -> Result<Config, ConfigError> {
    let raw: RawConfig = Figment::new()
        .merge(Env::prefixed("FLEET_"))
        .extract()
        .map_err(|e| ConfigError::Load(e.to_string()))?;

    let state_dir = match raw.state_dir {
        Some(dir) => PathBuf::from(dir),
        None => {
            let dir = default_state_dir()?;
            warn_if_cwd_state_orphaned(&dir);
            dir
        }
    };

    Ok(Config { state_dir, ram_lanes: raw.ram_lanes, review_cap: raw.review_cap })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_defaults_when_no_env_set() {
        let cfg = load().expect("defaults load");
        assert_eq!(cfg.review_cap, 3);
    }
}
