//! The on-disk shape of a target repo's `.fleet/gates.toml`, and reading it. Split from
//! `gate_config.rs` (which applies it to the registry) for the 80-line cap.
//!
//! `.fleet/` rather than a root-level `fleet.toml`: this repo already puts per-repo fleet inputs
//! there (`fleet-worker`'s `.fleet/agents.toml` and `.fleet/skills.toml`), and one directory of
//! fleet config beats a third top-level dotfile in someone else's repo.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where the file is discovered, relative to `--repo`. Named in error messages.
pub const RELATIVE: &str = ".fleet/gates.toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatesFile {
    /// Keyed by the gate's registry id, e.g. `[gates."unit tests"]`. An id with no entry keeps
    /// the committed default.
    #[serde(default)]
    pub gates: BTreeMap<String, GateOverride>,
}

/// What a repo may say about one gate. Deliberately NOT "whether to run it": a repo can say what
/// its unit-test command IS, it cannot say it has no unit tests (AGENTS.md rule 6 -- a check that
/// examined zero inputs fails; an opt-out would be a check that is cheaper to fake than satisfy).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateOverride {
    /// argv, executed with the repo as cwd. Must be non-empty.
    pub command: Vec<String>,
    /// The tool that must resolve before the gate runs. Defaults to the registry's own probe for
    /// this gate -- a Node repo mapping `unit tests` to npm wants `probe = "npm"`, not `cargo`.
    pub probe: Option<String>,
}

pub fn path(repo: &Path) -> PathBuf {
    repo.join(".fleet").join("gates.toml")
}

/// `Ok(None)` when the file is absent -- the overwhelmingly common case, and the one where
/// behaviour must stay byte-identical to the hardcoded registry. A file that exists but does not
/// parse is an error, never a silent fallback to the defaults it was written to replace.
pub fn load(repo: &Path) -> Result<Option<GatesFile>, String> {
    let path = path(repo);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    toml::from_str(&text).map(Some).map_err(|e| format!("{}: {e}", path.display()))
}
