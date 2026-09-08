//! Embedded default `.fleet/` config templates.
//!
//! Baked into the binary via `include_str!` at compile time so an *installed* `fleet-worker`
//! binary always has a fallback default -- `CARGO_MANIFEST_DIR` does not exist at runtime for an
//! installed binary, so these must not be read from disk relative to it.

/// Default `agents.toml` sample/starter content, also used as the fallback when a target repo
/// has no `.fleet/agents.toml` of its own.
pub const AGENTS_TOML: &str = include_str!("../../templates/agents.toml");

/// Default `skills.toml` sample/starter content, also used as the fallback when a target repo
/// has no `.fleet/skills.toml` of its own.
pub const SKILLS_TOML: &str = include_str!("../../templates/skills.toml");
