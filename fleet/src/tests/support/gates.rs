//! Test helpers for driving a real `fleet run` whose gates are supplied by the target repo's own
//! `.fleet/gates.toml`. Every gate is mapped to a `bash` one-liner that prints exactly the
//! denominator its registry parser reads, so the run is fast, hermetic, and needs no semgrep /
//! trivy / conftest / uv / cargo on the machine -- which is the whole point of the config: a repo
//! says what its gate commands ARE, it never says it has none.

use super::cmd;
use std::path::Path;

/// One `[gates."<id>"]` table per registry gate. `unit tests` deliberately mirrors the Node
/// mapping documented in `docs/GATES-CONFIG.md`: a non-cargo command, with its own probe.
const CONFIG: &str = r#"
[gates."unit tests"]
command = ["bash", "-c", "echo 'test result: ok. 3 passed; 0 failed; 0 ignored'"]
probe = "bash"

[gates."mutants"]
command = ["bash", "-c", "echo 'mutants: caught=2 total=2'"]
probe = "bash"

[gates."semgrep"]
command = ["bash", "-c", "echo '10 files scanned, 0 findings'"]
probe = "bash"

[gates."trivy"]
command = ["bash", "-c", "echo '0 secret findings across 4 reported targets'"]
probe = "bash"

[gates."recur"]
command = ["bash", "-c", "echo 'recur-gate: checked=5 flagged=0'"]
probe = "bash"

[gates."detectors"]
command = ["bash", "-c", "echo '7 detectors match the manifest (denominator: 7)'"]
probe = "bash"

[gates."policy"]
command = ["bash", "-c", "echo '-- 3 passed, 0 failed (denominator: 3 policies) --'"]
probe = "bash"

[gates."corpus"]
command = ["bash", "-c", "echo 'DENOMINATOR checked=9 total=9 caught=0'"]
probe = "bash"
"#;

pub fn gates_toml(repo: &Path) {
    std::fs::create_dir_all(repo.join(".fleet")).unwrap();
    std::fs::write(repo.join(".fleet").join("gates.toml"), CONFIG).unwrap();
}

/// A real `fleet run --json`, parsed. `FLEET_LANE_BUDGET_MB` pins the capacity preflight's RAM
/// component the same way `cmd()` pins its LOAD component: a test must not depend on how much
/// memory the dev box happens to have free.
pub fn run_json(state_dir: &Path, repo: &Path, task: &str) -> serde_json::Value {
    let out = cmd()
        .env("FLEET_STATE_DIR", state_dir)
        .env("FLEET_LANE_BUDGET_MB", "1")
        .args(["run", "--json", "--task", task, "--repo"])
        .arg(repo)
        .output()
        .expect("binary runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("`fleet run --json` did not print JSON ({e}):\nstdout={stdout}\nstderr={}",
            String::from_utf8_lossy(&out.stderr))
    })
}
