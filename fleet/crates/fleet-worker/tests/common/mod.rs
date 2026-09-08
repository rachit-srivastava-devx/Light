//! Shared fixtures for `fleet-worker`'s integration tests.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

/// Serialises tests that mutate process-global env vars (`FLEET_WORKER_TEST_CHILD_EXE`, `PATH`).
pub static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Lock `ENV_LOCK`, recovering from a poisoned lock (one earlier test panicking while it held
/// the lock must not cascade-fail every later test in this binary).
pub fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
}

pub fn init_repo() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "fw-test-{}-{}",
        std::process::id(),
        fleet_merge::unique_name("repo")
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let fleet_dir = dir.join(".fleet");
    std::fs::create_dir_all(&fleet_dir).unwrap();
    std::fs::write(fleet_dir.join("agents.toml"), AGENTS_TOML).unwrap();
    std::fs::write(fleet_dir.join("skills.toml"), SKILLS_TOML).unwrap();
    run(&dir, &["init", "-q"]);
    run(&dir, &["config", "user.email", "test@example.com"]);
    run(&dir, &["config", "user.name", "test"]);
    std::fs::write(dir.join("f.txt"), b"x").unwrap();
    run(&dir, &["add", "."]);
    run(&dir, &["commit", "-q", "-m", "init"]);
    dir
}

fn run(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git").arg("-C").arg(dir).args(args).status().unwrap();
    assert!(status.success());
}

const AGENTS_TOML: &str = r#"
[[agents]]
agent_id = "builder"
capabilities = ["write"]
skills = ["rust"]
"#;

const SKILLS_TOML: &str = r#"
[[skills]]
id = "rust"
capabilities = ["write"]
"#;

pub fn fixture_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fw-fixture-agent"))
}
