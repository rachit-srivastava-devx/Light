//! `Config`'s typed load error, split out of `config.rs` for the 80-line cap.

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config load failed: {0}")]
    Load(String),
    #[error(
        "cannot resolve a default state dir: $HOME is not set. Set FLEET_STATE_DIR explicitly, \
         or export HOME, and retry."
    )]
    HomeUnset,
}
