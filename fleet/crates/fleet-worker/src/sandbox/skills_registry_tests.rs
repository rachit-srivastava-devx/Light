use super::*;
use crate::sandbox::agent_registry::AgentFacts;
use crate::sandbox::templates::SKILLS_TOML as EMBEDDED_SKILLS_TOML;
use std::fs;

fn write_skills(dir: &Path, text: &str) {
    let fleet_dir = dir.join(".fleet");
    fs::create_dir_all(&fleet_dir).unwrap();
    fs::write(fleet_dir.join("skills.toml"), text).unwrap();
}

#[test]
fn unresolved_skill_is_caught() {
    let dir = tempfile::tempdir().unwrap();
    write_skills(dir.path(), "[[skills]]\nid = \"rust\"\ncapabilities = [\"write\"]\n");
    let agent = AgentFacts {
        capabilities: vec!["write".to_string()],
        skills: vec!["debugging".to_string()],
    };
    let err = resolve_skills(dir.path(), &agent).unwrap_err();
    assert_eq!(err, ProvisionError::UnresolvedSkill("debugging".to_string()));
}

#[test]
fn fully_resolved_registry_passes() {
    let dir = tempfile::tempdir().unwrap();
    write_skills(dir.path(), "[[skills]]\nid = \"rust\"\ncapabilities = [\"write\"]\n");
    let agent = AgentFacts {
        capabilities: vec!["write".to_string()],
        skills: vec!["rust".to_string()],
    };
    let resolved = resolve_skills(dir.path(), &agent).unwrap();
    assert!(resolved.contains("rust"));
}

#[test]
fn repo_committed_skills_toml_beats_embedded_default() {
    // The repo declares a skill ("bespoke") that the embedded default template does not have.
    // Resolving it must succeed, proving the repo's own file -- not the fallback -- was read.
    let dir = tempfile::tempdir().unwrap();
    write_skills(dir.path(), "[[skills]]\nid = \"bespoke\"\ncapabilities = []\n");
    let agent = AgentFacts { capabilities: vec![], skills: vec!["bespoke".to_string()] };
    let resolved = resolve_skills(dir.path(), &agent).unwrap();
    assert!(resolved.contains("bespoke"));
    assert!(!EMBEDDED_SKILLS_TOML.contains("bespoke"));
}

#[test]
fn missing_fleet_dir_falls_back_to_embedded_default() {
    // No .fleet/ tree at all -- resolving a skill known only to the embedded template ("rust")
    // must still succeed, proving the fallback default was used.
    let dir = tempfile::tempdir().unwrap();
    assert!(EMBEDDED_SKILLS_TOML.contains("id = \"rust\""));
    let agent = AgentFacts {
        capabilities: vec!["write".to_string()],
        skills: vec!["rust".to_string()],
    };
    let resolved = resolve_skills(dir.path(), &agent).unwrap();
    assert!(resolved.contains("rust"));
}
