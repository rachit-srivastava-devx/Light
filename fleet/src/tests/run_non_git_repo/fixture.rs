//! The `.fleet/gates.toml` this contract drives its gates with. Deliberately NOT
//! `support::gates::gates_toml`: every command here is guarded by a marker file that exists only
//! inside the target directory, so a gate that ran against the wrong cwd exits 9 instead of
//! quietly printing a denominator it never earned. "The run exited 0" is a proxy; "the gate found
//! the marker that only lives in `--repo`" is the property (PRINCIPLES: a check cheaper to fake
//! than to satisfy will be faked).
//!
//! Also holds the suite's plumbing (`run`, `why`, `stage`) so `main.rs` is assertions only.

use super::support::{cmd, gates::gates_toml, scratch_repo_staged};
use serde_json::Value;
use std::path::Path;
use std::process::Output;

/// One real `fleet run --json` against `repo`, with `flags` inserted before `--repo`. Each call
/// gets its own state dir so no test inherits another's step log (crash-resume would otherwise
/// mark stages "resumed" and hide the outcome under test).
pub fn run(repo: &Path, flags: &[&str]) -> (Output, Option<Value>) {
    let state = tempfile::tempdir().expect("tempdir");
    let out = cmd()
        .env("FLEET_STATE_DIR", state.path())
        .env("FLEET_LANE_BUDGET_MB", "1")
        .args(["run", "--json", "--task", "add-a-hello-function"])
        .args(flags)
        .arg("--repo")
        .arg(repo)
        .output()
        .expect("binary runs");
    let json = serde_json::from_slice(&out.stdout).ok();
    (out, json)
}

/// Every assertion message carries the real exit code and both streams -- a failing contract must
/// name what actually happened, not just that it did not match.
pub fn why(out: &Output) -> String {
    format!(
        "exit={:?}\nstdout={}\nstderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// One named `StageRecord` out of the outcome's `stages` array.
pub fn stage<'a>(v: &'a Value, name: &str) -> &'a Value {
    v["stages"]
        .as_array()
        .unwrap_or_else(|| panic!("outcome has no stages array: {v}"))
        .iter()
        .find(|s| s["stage"] == name)
        .unwrap_or_else(|| panic!("no {name} stage record in {v}"))
}

/// A git worktree with a non-empty stage and the shared gate config -- the control arm for every
/// "the default path is unchanged" assertion.
pub fn git_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    scratch_repo_staged(dir.path());
    gates_toml(dir.path());
    dir
}

/// Lives inside the repo. `RealRunner` sets each gate's cwd to `--repo`, so a gate executing
/// anywhere else cannot see this file.
pub const MARKER: &str = ".fleet/cwd-marker";

/// One line per registry gate, each the exact string that gate's registry parser reads a
/// denominator out of. Same payloads as `support::gates`, so a failure here is about cwd or
/// about `--no-git`, never about parser drift.
const ECHOES: &[(&str, &str)] = &[
    (
        "unit tests",
        "test result: ok. 3 passed; 0 failed; 0 ignored",
    ),
    ("mutants", "mutants: caught=2 total=2"),
    ("semgrep", "10 files scanned, 0 findings"),
    ("trivy", "0 secret findings across 4 reported targets"),
    ("recur", "recur-gate: checked=5 flagged=0"),
    (
        "detectors",
        "7 detectors match the manifest (denominator: 7)",
    ),
    (
        "policy",
        "-- 3 passed, 0 failed (denominator: 3 policies) --",
    ),
    ("corpus", "DENOMINATOR checked=9 total=9 caught=0"),
];

/// Writes `.fleet/gates.toml` + the cwd marker into `repo`. Never runs `git init` -- the whole
/// point of the directory under test is that it is not a git worktree.
pub fn gates_toml_cwd_proof(repo: &Path) {
    let dir = repo.join(".fleet");
    std::fs::create_dir_all(&dir).expect("mkdir .fleet");
    std::fs::write(repo.join(MARKER), b"").expect("write cwd marker");
    let mut out = String::new();
    for (id, echo) in ECHOES {
        out.push_str(&format!(
            "[gates.\"{id}\"]\ncommand = [\"bash\", \"-c\", \
             \"test -f {MARKER} || exit 9; echo '{echo}'\"]\nprobe = \"bash\"\n\n"
        ));
    }
    std::fs::write(dir.join("gates.toml"), out).expect("write gates.toml");
}

/// A plain local project directory: real files, real gate config, **no `.git` anywhere**.
pub fn plain_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("app.py"), "def main():\n    pass\n").expect("write app.py");
    std::fs::write(dir.path().join("README.md"), "# plain project\n").expect("write README");
    gates_toml_cwd_proof(dir.path());
    dir
}
