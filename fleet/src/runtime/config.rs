//! Layered config: env vars over defaults, one typed `Config` struct, replacing the ad hoc
//! `env::var("FLEET_...")` reads scattered through the old `main.rs`/`route.rs`/`meter.rs`.
//! Deviation from BLUEPRINT §7: only the `env` figment provider is wired (no file layer yet) --
//! flagged, not silently dropped; add a `Toml::file(...)` provider when a config file format is
//! decided.

use figment::providers::Env;
use figment::Figment;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_state")]
    pub state_dir: String,
    #[serde(default)]
    pub ram_lanes: Option<usize>,
    #[serde(default = "default_review_cap")]
    pub review_cap: usize,
}

fn default_state() -> String {
    ".fleet-state".to_string()
}

fn default_review_cap() -> usize {
    3
}

#[derive(Debug, thiserror::Error)]
#[error("config load failed: {0}")]
pub struct ConfigError(String);

pub fn load() -> Result<Config, ConfigError> {
    Figment::new()
        .merge(Env::prefixed("FLEET_"))
        .extract()
        .map_err(|e| ConfigError(e.to_string()))
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
