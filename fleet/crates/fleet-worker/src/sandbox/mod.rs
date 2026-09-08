//! Hermetic sandbox provisioning: resolve one agent's skill/MCP/system-prompt seed data from
//! the repo's committed `.fleet/` tree, and build the per-lane `HOME`/`XDG_*` env.

pub mod agent_registry;
mod config_source;
pub mod hermetic_env;
pub mod manifest;
pub mod scaffold;
mod skills_registry;
mod templates;

use std::collections::BTreeSet;
use std::path::Path;

/// The committed skill registry resolved against one agent's declared capabilities --
/// hermetic-provisioning seed data fed into the sandbox before spawn.
pub struct HermeticProvision {
    pub skill_ids: BTreeSet<String>,
    pub mcp_tool_manifest: serde_json::Value,
    pub system_prompt: String,
}

/// Why hermetic provisioning could not assemble a `HermeticProvision` for one agent/lease.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProvisionError {
    #[error("repo .fleet/ tree is missing required file: {0}")]
    MissingFleetFile(&'static str),
    #[error("skill {0:?} is declared by the agent but absent from the committed registry")]
    UnresolvedSkill(String),
    #[error("agent {0:?} declares skill {1:?} but is missing required capabilities: {2:?}")]
    MissingCapabilities(String, String, Vec<String>),
}

/// Resolve one agent's hermetic provision from the repo's committed `.fleet/` tree. Never reads
/// `$HOME` or any path outside `repo` -- every input is repo-committed and version-controlled.
pub fn resolve_hermetic_provision(
    repo: &Path,
    agent_id: &str,
) -> Result<HermeticProvision, ProvisionError> {
    if agent_id.contains('/') || agent_id.contains("..") || agent_id.trim().is_empty() {
        return Err(ProvisionError::MissingFleetFile("agents.toml"));
    }
    let agent = agent_registry::load_agent(repo, agent_id)?;
    let skill_ids = skills_registry::resolve_skills(repo, &agent)?;
    let lease = format!(".worktrees/{agent_id}/**");
    let mcp_tool_manifest = manifest::manifest_for_lease(&lease)?;
    let system_prompt = format!(
        "agent={agent_id} skills={skill_ids:?} capabilities={:?}",
        agent.capabilities
    );
    Ok(HermeticProvision { skill_ids, mcp_tool_manifest, system_prompt })
}
