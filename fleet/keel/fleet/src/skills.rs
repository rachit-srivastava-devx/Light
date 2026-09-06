//! Committed skill registry and resolution against agent declarations.

use crate::agent::{Agent, Registry as AgentRegistry};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const EXIT_ENV: i32 = 3;
const EXIT_INVARIANT: i32 = 6;

#[derive(Clone, Debug, Deserialize)]
struct SkillFile {
    id: String,
    purpose: String,
    capabilities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct RegistryFile {
    #[serde(default)]
    skills: Vec<SkillFile>,
}

#[derive(Clone, Debug)]
pub struct Skill {
    inner: SkillFile,
}

impl Skill {
    pub fn id(&self) -> &str {
        &self.inner.id
    }

    pub fn purpose(&self) -> &str {
        &self.inner.purpose
    }

    pub fn capabilities(&self) -> &[String] {
        &self.inner.capabilities
    }
}

#[derive(Clone, Debug)]
pub struct Registry {
    skills: Vec<Skill>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolutionStatus {
    Resolved,
    UnresolvedMissingRegistryEntry,
    UnresolvedMissingCapabilities(Vec<String>),
}

#[derive(Clone, Debug)]
pub struct Resolution {
    pub agent_id: String,
    pub skill_id: String,
    pub status: ResolutionStatus,
}

#[derive(Clone, Debug)]
pub struct ResolutionReport {
    pub registered: usize,
    pub resolved: usize,
    pub checked: usize,
    pub total: usize,
    pub resolutions: Vec<Resolution>,
}

impl ResolutionReport {
    pub fn passes(&self) -> bool {
        self.registered > 0
            && self.checked > 0
            && self.checked == self.total
            && self.resolved == self.total
    }

    pub fn unresolved(&self) -> impl Iterator<Item = &Resolution> {
        self.resolutions
            .iter()
            .filter(|resolution| resolution.status != ResolutionStatus::Resolved)
    }
}

impl Registry {
    fn from_toml(text: &str) -> Result<Self, i32> {
        let parsed: RegistryFile = toml::from_str(text).map_err(|_| EXIT_INVARIANT)?;
        let mut ids = BTreeSet::new();
        if parsed.skills.iter().any(|skill| {
            skill.id.trim().is_empty()
                || skill.purpose.trim().is_empty()
                || skill.capabilities.is_empty()
                || skill
                    .capabilities
                    .iter()
                    .any(|capability| capability.trim().is_empty())
                || !ids.insert(skill.id.as_str())
        }) {
            return Err(EXIT_INVARIANT);
        }
        Ok(Self {
            skills: parsed
                .skills
                .into_iter()
                .map(|inner| Skill { inner })
                .collect(),
        })
    }

    pub fn load(path: &Path) -> Result<Self, i32> {
        let text = fs::read_to_string(path).map_err(|_| EXIT_ENV)?;
        Self::from_toml(&text)
    }

    pub fn load_default() -> Result<Self, i32> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("skills.toml");
        Self::load(&path)
    }

    pub fn skills(&self) -> &[Skill] {
        &self.skills
    }

    fn resolve(&self, agent: &Agent, skill_id: &str) -> ResolutionStatus {
        let Some(skill) = self.skills.iter().find(|skill| skill.id() == skill_id) else {
            return ResolutionStatus::UnresolvedMissingRegistryEntry;
        };
        let missing = skill
            .capabilities()
            .iter()
            .filter(|required| !agent.capabilities().contains(required))
            .cloned()
            .collect::<Vec<_>>();
        if missing.is_empty() {
            ResolutionStatus::Resolved
        } else {
            ResolutionStatus::UnresolvedMissingCapabilities(missing)
        }
    }

    pub fn evaluate(&self, agents: &AgentRegistry) -> ResolutionReport {
        let resolutions = agents
            .agents()
            .iter()
            .flat_map(|agent| {
                agent.skills().iter().map(|skill_id| Resolution {
                    agent_id: agent.agent_id().to_string(),
                    skill_id: skill_id.clone(),
                    status: self.resolve(agent, skill_id),
                })
            })
            .collect::<Vec<_>>();
        let total = resolutions.len();
        let resolved = resolutions
            .iter()
            .filter(|resolution| resolution.status == ResolutionStatus::Resolved)
            .count();
        ResolutionReport {
            registered: self.skills.len(),
            resolved,
            checked: total,
            total,
            resolutions,
        }
    }

    pub fn require_for_agent(
        &self,
        agents: &AgentRegistry,
        agent_id: &str,
        required_skills: &[&str],
    ) -> Result<(), i32> {
        if required_skills.is_empty() {
            return Err(EXIT_INVARIANT);
        }
        let agent = agents
            .agents()
            .iter()
            .find(|agent| agent.agent_id() == agent_id)
            .ok_or(EXIT_INVARIANT)?;
        if required_skills.iter().all(|skill_id| {
            agent.skills().iter().any(|declared| declared == skill_id)
                && self.resolve(agent, skill_id) == ResolutionStatus::Resolved
        }) {
            Ok(())
        } else {
            Err(EXIT_INVARIANT)
        }
    }
}

pub fn command(check: bool) -> Result<(), i32> {
    let skills = Registry::load_default()?;
    let agents = AgentRegistry::load_default()?;
    for skill in skills.skills() {
        println!(
            "skill={} purpose={:?} requires={:?}",
            skill.id(),
            skill.purpose(),
            skill.capabilities()
        );
    }
    let report = skills.evaluate(&agents);
    for resolution in &report.resolutions {
        match &resolution.status {
            ResolutionStatus::Resolved => println!(
                "agent={} skill={} RESOLVED",
                resolution.agent_id, resolution.skill_id
            ),
            ResolutionStatus::UnresolvedMissingRegistryEntry => println!(
                "agent={} skill={} UNRESOLVED reason=missing-registry-entry",
                resolution.agent_id, resolution.skill_id
            ),
            ResolutionStatus::UnresolvedMissingCapabilities(missing) => println!(
                "agent={} skill={} UNRESOLVED missing-capabilities={:?}",
                resolution.agent_id, resolution.skill_id, missing
            ),
        }
    }
    println!(
        "{} of {} declared skills resolve; denominator={{checked:{}, total:{}}}; registered={}",
        report.resolved, report.total, report.checked, report.total, report.registered
    );
    if report.registered == 0 || report.checked == 0 {
        eprintln!("fleet: skill check failed: zero inputs examined");
        return Err(EXIT_INVARIANT);
    }
    if check && !report.passes() {
        let names = report
            .unresolved()
            .map(|resolution| format!("{}:{}", resolution.agent_id, resolution.skill_id))
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("fleet: unresolved declared skills: {names}");
        return Err(EXIT_INVARIANT);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Registry;
    use crate::agent::Registry as AgentRegistry;

    const AGENTS: &str = r#"
[[agents]]
agent_id = "builder"
role = "builder"
charter = "Build"
capabilities = ["read", "write"]
skills = ["rust", "debugging"]
"#;

    #[test]
    fn unresolved_skill_is_caught() {
        let agents = AgentRegistry::from_toml(AGENTS).unwrap();
        let skills = Registry::from_toml(
            r#"[[skills]]
id = "rust"
purpose = "Implement Rust"
capabilities = ["write"]
"#,
        )
        .unwrap();
        let report = skills.evaluate(&agents);
        assert_eq!((report.resolved, report.total), (1, 2));
        assert!(!report.passes());
        assert_eq!(report.unresolved().count(), 1);
    }

    #[test]
    fn fully_resolved_registry_passes() {
        let agents = AgentRegistry::from_toml(AGENTS).unwrap();
        let skills = Registry::from_toml(
            r#"
[[skills]]
id = "rust"
purpose = "Implement Rust"
capabilities = ["write"]
[[skills]]
id = "debugging"
purpose = "Diagnose failures"
capabilities = ["read"]
"#,
        )
        .unwrap();
        let report = skills.evaluate(&agents);
        assert_eq!((report.resolved, report.total), (2, 2));
        assert!(report.passes());
    }

    #[test]
    fn empty_registry_fails() {
        let agents = AgentRegistry::from_toml(AGENTS).unwrap();
        let skills = Registry::from_toml("").unwrap();
        let report = skills.evaluate(&agents);
        assert_eq!(report.registered, 0);
        assert!(!report.passes());
    }
}
