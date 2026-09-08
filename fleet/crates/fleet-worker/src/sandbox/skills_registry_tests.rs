use super::*;
use crate::sandbox::agent_registry::AgentFacts;

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
