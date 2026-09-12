//! `fleet run --repo a --repo b` -- one task_id, N repos, aggregated verdict. The Frido
//! workspace's three sibling repos (posx-frido-{store,backend,admin}) is the shape here: one
//! client feature touches 2-3 of them, so `--repo` accepts a list. Sequential (single-machine
//! concurrency cap), one receipt per (task_id, repo) pair, worst-of-set exit code.

mod support;
use serde_json::Value;
use support::gates::gates_toml;
use support::{cmd, scratch_repo_staged};

/// Two git-init'd tempdirs -> two receipts, one per repo, same task_id.
#[test]
fn two_repos_one_task_id_two_receipts() {
    let state_dir = tempfile::tempdir().unwrap();
    let repo_a = tempfile::tempdir().unwrap();
    let repo_b = tempfile::tempdir().unwrap();
    scratch_repo_staged(repo_a.path());
    gates_toml(repo_a.path());
    scratch_repo_staged(repo_b.path());
    gates_toml(repo_b.path());

    let out = cmd()
        .env("FLEET_STATE_DIR", state_dir.path())
        .env("FLEET_LANE_BUDGET_MB", "1")
        .args(["run", "--json", "--task", "multi-repo-task", "--repo"])
        .arg(repo_a.path())
        .arg("--repo")
        .arg(repo_b.path())
        .output()
        .expect("binary runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "both green -> exit 0. stdout={stdout} stderr={}",
        String::from_utf8_lossy(&out.stderr));

    // Two JSON objects concatenated, one per repo. Split on the pretty-printer's own boundary.
    let objs: Vec<Value> = serde_json::Deserializer::from_str(&stdout)
        .into_iter::<Value>()
        .collect::<Result<_, _>>()
        .unwrap_or_else(|e| panic!("expected N JSON objects, got err {e}: {stdout}"));
    assert_eq!(objs.len(), 2, "one JSON outcome per repo: {stdout}");
    for v in &objs {
        assert_eq!(v["task"], "multi-repo-task", "shared task_id correlates the set: {v}");
    }
}

/// Mixed pass/fail -> aggregated exit code = worst (non-zero).
#[test]
fn mixed_pass_fail_aggregates_to_worst() {
    let state_dir = tempfile::tempdir().unwrap();
    let good = tempfile::tempdir().unwrap();
    let bad = tempfile::tempdir().unwrap();
    scratch_repo_staged(good.path());
    gates_toml(good.path());
    // `bad` is a plain dir -- ensure_repo/preflight refuses it (not a readable git repo).
    let out = cmd()
        .env("FLEET_STATE_DIR", state_dir.path())
        .env("FLEET_LANE_BUDGET_MB", "1")
        .args(["run", "--task", "mixed-task", "--repo"])
        .arg(good.path())
        .arg("--repo")
        .arg("/does/not/exist/at/all/xyz")
        .output()
        .expect("binary runs");
    assert!(!out.status.success(), "worst-of-set is a refusal, exit != 0. stderr={}",
        String::from_utf8_lossy(&out.stderr));
}

/// Non-git repo -> existing preflight refusal message survives (regression pin).
#[test]
fn non_git_repo_preflight_message_pinned() {
    let state_dir = tempfile::tempdir().unwrap();
    let out = cmd()
        .env("FLEET_STATE_DIR", state_dir.path())
        .env("FLEET_LANE_BUDGET_MB", "1")
        .args(["run", "--task", "t", "--repo", "/no/such/repo/here/xyz"])
        .output()
        .expect("binary runs");
    assert!(!out.status.success(), "missing repo refuses");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("fleet:"), "typed refusal via main's error printer: {stderr}");
}

/// Single `--repo` invocation unchanged (regression pin).
#[test]
fn single_repo_back_compat() {
    let state_dir = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    scratch_repo_staged(repo.path());
    gates_toml(repo.path());
    let out = cmd()
        .env("FLEET_STATE_DIR", state_dir.path())
        .env("FLEET_LANE_BUDGET_MB", "1")
        .args(["run", "--json", "--task", "single-task", "--repo"])
        .arg(repo.path())
        .output()
        .expect("binary runs");
    assert!(out.status.success(), "single --repo still green: stderr={}",
        String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Exactly one JSON object -- unchanged from pre-multi-repo behaviour.
    let objs: Vec<Value> = serde_json::Deserializer::from_str(&stdout)
        .into_iter::<Value>()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(objs.len(), 1, "single --repo emits one outcome: {stdout}");
    assert_eq!(objs[0]["task"], "single-task");
}
