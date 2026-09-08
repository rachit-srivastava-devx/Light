//! Committed skill registry + resolution against one agent's declared capabilities. Ported near-
//! verbatim from `skills.rs`'s resolution logic, loading from `.fleet/skills.toml` (the target
//! repo's own copy wins; this crate's embedded template is the fallback default) -- the HERMETIC
//! requirement is that the sandbox seed comes only from `.fleet/` or this crate's own binary.

use crate::sandbox::agent_registry::AgentFacts;
use crate::sandbox::config_source::read_with_fallback;
use crate::sandbox::templates::SKILLS_TOML;
use crate::ProvisionError;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Clone, Debug, Deserialize)]
struct SkillFile {
    id: String,
    capabilities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct RegistryFile {
    #[serde(default)]
    skills: Vec<SkillFile>,
}

/// Resolve every `skill_id` an agent declares against `.fleet/skills.toml`'s committed set,
/// returning the resolved ids plus, for the first unresolved skill found, the failure detail.
pub fn resolve_skills(
    repo: &Path,
    agent: &AgentFacts,
) -> Result<BTreeSet<String>, ProvisionError> {
    let (text, _source) = read_with_fallback(repo, "skills.toml", SKILLS_TOML)?;
    let parsed: RegistryFile =
        toml::from_str(&text).map_err(|_| ProvisionError::MissingFleetFile("skills.toml"))?;
    let mut resolved = BTreeSet::new();
    for skill_id in &agent.skills {
        let Some(skill) = parsed.skills.iter().find(|s| &s.id == skill_id) else {
            return Err(ProvisionError::UnresolvedSkill(skill_id.clone()));
        };
        let missing: Vec<String> = skill
            .capabilities
            .iter()
            .filter(|required| !agent.capabilities.contains(required))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(ProvisionError::MissingCapabilities(
                "agent".to_string(),
                skill_id.clone(),
                missing,
            ));
        }
        resolved.insert(skill_id.clone());
    }
    Ok(resolved)
}

#[cfg(test)]
#[path = "skills_registry_tests.rs"]
mod tests;
