//! Resolution order shared by `agent_registry` and `skills_registry`: the target repo's
//! committed `.fleet/<name>` wins; the crate's embedded template is the fallback default when
//! the repo has none. A file that *exists* but cannot be read is a real error, not a fallback
//! trigger -- silently swallowing a broken repo file would hide the actual problem.

use crate::ProvisionError;
use std::fs;
use std::path::Path;

/// Where a resolved config file's text came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigSource {
    /// Read from the target repo's `.fleet/<name>`.
    Repo,
    /// The repo had no `.fleet/<name>`; this crate's embedded template was used instead.
    EmbeddedDefault,
}

/// Resolve `repo/.fleet/<name>`'s text: the repo's own file if present, else `default_text`.
pub fn read_with_fallback(
    repo: &Path,
    name: &'static str,
    default_text: &'static str,
) -> Result<(String, ConfigSource), ProvisionError> {
    let path = repo.join(".fleet").join(name);
    if !path.exists() {
        return Ok((default_text.to_string(), ConfigSource::EmbeddedDefault));
    }
    let text =
        fs::read_to_string(&path).map_err(|_| ProvisionError::MissingFleetFile(name))?;
    Ok((text, ConfigSource::Repo))
}
