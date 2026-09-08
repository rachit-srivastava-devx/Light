//! Write a starter `.fleet/agents.toml` + `.fleet/skills.toml` into a target repo from this
//! crate's embedded templates.
//!
//! Closes a real usability hole found in E2E testing: `fleet swarm`/`fleet agents` fail with
//! "repo .fleet/ tree is missing required file: agents.toml" and there was no way for a user to
//! create one. This gives them a valid starting point instead of a dead end.

use crate::sandbox::templates::{AGENTS_TOML, SKILLS_TOML};
use std::fs;
use std::path::Path;

/// Why `scaffold_fleet_dir` could not write the starter `.fleet/` config.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ScaffoldError {
    #[error("{0} already exists at {1}; scaffold does not overwrite an existing .fleet/ config")]
    AlreadyExists(&'static str, String),
    #[error("could not write {0}: {1}")]
    Io(String, String),
}

/// Write `.fleet/agents.toml` and `.fleet/skills.toml` under `repo` from the embedded default
/// templates. Refuses -- writing neither file -- if either already exists: scaffold seeds a
/// missing tree, it never clobbers a target repo's real config.
pub fn scaffold_fleet_dir(repo: &Path) -> Result<(), ScaffoldError> {
    let fleet_dir = repo.join(".fleet");
    check_absent(&fleet_dir, "agents.toml")?;
    check_absent(&fleet_dir, "skills.toml")?;
    fs::create_dir_all(&fleet_dir).map_err(|e| io_err(&fleet_dir, &e))?;
    write_file(&fleet_dir, "agents.toml", AGENTS_TOML)?;
    write_file(&fleet_dir, "skills.toml", SKILLS_TOML)?;
    Ok(())
}

fn check_absent(fleet_dir: &Path, name: &'static str) -> Result<(), ScaffoldError> {
    let path = fleet_dir.join(name);
    if path.exists() {
        return Err(ScaffoldError::AlreadyExists(name, path.display().to_string()));
    }
    Ok(())
}

fn write_file(fleet_dir: &Path, name: &'static str, contents: &str) -> Result<(), ScaffoldError> {
    let path = fleet_dir.join(name);
    fs::write(&path, contents).map_err(|e| io_err(&path, &e))
}

fn io_err(path: &Path, err: &std::io::Error) -> ScaffoldError {
    ScaffoldError::Io(path.display().to_string(), err.to_string())
}

#[cfg(test)]
#[path = "scaffold_tests.rs"]
mod tests;
