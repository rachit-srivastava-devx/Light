//! Minimal agent-identity lookup against the repo-committed `.fleet/agents.toml` tree.
//!
//! This is deliberately NOT `agent.rs`'s full `Registry`/`Assignment<S>` machinery (BLUEPRINT.md
//! flags `Assignment<S>` as out of scope, belonging to `fleet-lifecycle`) -- `fleet-worker` only
//! needs one agent's declared `capabilities`/`skills` to resolve its hermetic provision.

use crate::ProvisionError;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Deserialize)]
struct AgentFile {
    agent_id: String,
    capabilities: Vec<String>,
    skills: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct RegistryFile {
    #[serde(default)]
    agents: Vec<AgentFile>,
}

/// One agent's declared capabilities/skills, resolved from `.fleet/agents.toml`.
pub struct AgentFacts {
    pub capabilities: Vec<String>,
    pub skills: Vec<String>,
}

/// Load `.fleet/agents.toml` under `repo` and find `agent_id`. Never reads any path outside
/// `repo` -- every input is repo-committed (PLAYBOOK.md rule 8).
pub fn load_agent(repo: &Path, agent_id: &str) -> Result<AgentFacts, ProvisionError> {
    let path = repo.join(".fleet").join("agents.toml");
    let text = fs::read_to_string(&path)
        .map_err(|_| ProvisionError::MissingFleetFile("agents.toml"))?;
    let parsed: RegistryFile =
        toml::from_str(&text).map_err(|_| ProvisionError::MissingFleetFile("agents.toml"))?;
    let agent = parsed
        .agents
        .into_iter()
        .find(|agent| agent.agent_id == agent_id)
        // No dedicated "agent not declared" variant in the §3 contract -- the closest fit is
        // "the committed tree is missing what this call needed."
        .ok_or(ProvisionError::MissingFleetFile("agents.toml"))?;
    Ok(AgentFacts {
        capabilities: agent.capabilities,
        skills: agent.skills,
    })
}
