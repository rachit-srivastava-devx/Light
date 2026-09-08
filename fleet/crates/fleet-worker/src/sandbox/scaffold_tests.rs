use super::*;
use crate::sandbox::agent_registry::load_agent;

#[test]
fn scaffold_writes_files_that_then_parse() {
    let dir = tempfile::tempdir().unwrap();
    scaffold_fleet_dir(dir.path()).unwrap();

    assert!(dir.path().join(".fleet/agents.toml").exists());
    assert!(dir.path().join(".fleet/skills.toml").exists());

    // The scaffolded template names "builder" -- loading it back through the real resolution
    // path (not just parsing raw TOML) proves the scaffold output is valid, not merely present.
    let agent = load_agent(dir.path(), "builder").unwrap();
    assert!(agent.capabilities.contains(&"write".to_string()));
}

#[test]
fn scaffold_refuses_to_overwrite_existing_config() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".fleet")).unwrap();
    std::fs::write(dir.path().join(".fleet/agents.toml"), "custom").unwrap();

    let err = scaffold_fleet_dir(dir.path()).unwrap_err();
    assert!(matches!(err, ScaffoldError::AlreadyExists("agents.toml", _)));

    // Untouched: scaffold must not have written skills.toml either.
    assert!(!dir.path().join(".fleet/skills.toml").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".fleet/agents.toml")).unwrap(),
        "custom"
    );
}
