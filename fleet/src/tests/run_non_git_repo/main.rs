//! CONTRACT (pre-authored, T1 -- RED until implemented; do not edit to make it pass, hard rule 1).
//!
//! **Gap.** `fleet run --repo <plain directory>` refuses at intake with exit 3 ("not a git
//! repository"), so fleet cannot be pointed at a local, non-git project at all. Goal item 8 wants
//! the opposite. `ensure_repo` -> `ensure_git_worktree` is the only thing in the way: `fleet run`'s
//! entire git surface is two files (`verify_repo.rs`, `merge_stage.rs`) -- Event/Classify/Scan/
//! Plan/Dispatch touch no git, and Verify runs the gate table with `--repo` as cwd.
//!
//! **Interface this suite pins.**
//! * `fleet run --no-git` (bool, default false) -- declares `--repo` is a plain directory.
//!   OPT-IN ONLY: without it a non-git `--repo` still refuses with exit 3, because the guard's
//!   original job (a mistyped path, or a parent of sibling checkouts) is still real. Auto-detect
//!   is out: it would require editing `verify_repo_tests.rs`, which hard rule 1 forbids.
//! * Intake skips only the git-worktree check; `ensure_repo_readable` still applies.
//! * `Merge` is SKIPPED, not passed and not failed -- it is the one stage that genuinely needs
//!   git. Its `StageRecord` outcome is `"skip"` with `elapsed_ms` absent (hard rule 9).
//! * `--json` carries `"repo_mode": "git" | "no-git"` so the machine receipt says which stages
//!   were structurally unavailable. A run that skipped merge must not read as a run that merged.
//! * `--no-git` against a real git worktree is legal (declines merge, no error) so a wrapper
//!   script may pass it unconditionally.
//!
//! **Out of scope for this node:** `swarm`/`gate`/`oracle` intake, `crates/merge` worktree
//! isolation, `crates/builder` spawn/change-detect, `fleet pr`, rollback. All inherently git.

mod fixture;
#[path = "../support/mod.rs"]
mod support;

use fixture::{git_dir, plain_dir, run, stage, why};
use serde_json::Value;

/// The headline: a directory with files and no `.git` anywhere runs the pipeline green.
#[test]
fn plain_non_git_directory_runs_green_with_the_flag() {
    let repo = plain_dir();
    let (out, json) = run(repo.path(), &["--no-git"]);
    assert!(
        out.status.success(),
        "--no-git must accept a plain dir: {}",
        why(&out)
    );
    let v = json.unwrap_or_else(|| panic!("--json must print one outcome: {}", why(&out)));
    assert_eq!(v["final_stage"], "Teach", "a green run ends at Teach: {v}");
    assert_eq!(v["refusal"], Value::Null, "no refusal on a green run: {v}");
}

/// Merge did not happen, so the record must not say it did. A fabricated "pass" here is the
/// single worst outcome this suite exists to prevent.
#[test]
fn merge_is_recorded_as_skipped_never_as_passed() {
    let repo = plain_dir();
    let (out, json) = run(repo.path(), &["--no-git"]);
    let v = json.unwrap_or_else(|| panic!("--json must print one outcome: {}", why(&out)));
    let merge = stage(&v, "Merge");
    assert_eq!(
        merge["outcome"], "skip",
        "Merge must be skipped, not passed/failed: {merge}"
    );
    assert_eq!(
        merge["elapsed_ms"],
        Value::Null,
        "a skipped stage spent no time: {merge}"
    );
}

/// The receipt names its own mode, both ways: a consumer must be able to tell "merged" from
/// "merge was never available".
#[test]
fn json_declares_repo_mode_on_both_paths() {
    let plain = plain_dir();
    let (out, json) = run(plain.path(), &["--no-git"]);
    let v = json.unwrap_or_else(|| panic!("--json must print one outcome: {}", why(&out)));
    assert_eq!(
        v["repo_mode"], "no-git",
        "no-git run must declare its mode: {v}"
    );

    let git = git_dir();
    let (out, json) = run(git.path(), &[]);
    let v = json.unwrap_or_else(|| panic!("--json must print one outcome: {}", why(&out)));
    assert_eq!(
        v["repo_mode"], "git",
        "the default path must declare git mode: {v}"
    );
    assert_eq!(
        stage(&v, "Merge")["outcome"],
        "pass",
        "git path still merges: {v}"
    );
}

/// The gates really executed in `--repo`, not in the test process's cwd. Every gate command is
/// guarded by a marker that only exists there, and the denominator proves the echo ran.
#[test]
fn gates_execute_with_the_plain_directory_as_cwd() {
    let repo = plain_dir();
    let (out, json) = run(repo.path(), &["--no-git"]);
    let v = json.unwrap_or_else(|| panic!("--json must print one outcome: {}", why(&out)));
    let gates = v["gates"]
        .as_array()
        .unwrap_or_else(|| panic!("no gates array: {v}"));
    let unit = gates
        .iter()
        .find(|g| g["id"] == "unit tests")
        .unwrap_or_else(|| panic!("no 'unit tests' gate record: {v}"));
    assert_eq!(
        unit["verdict"], "pass",
        "marker-guarded gate passed => right cwd: {unit}"
    );
    assert_eq!(
        unit["checked"], 3,
        "denominator published, not null: {unit}"
    );
    assert_eq!(unit["total"], 3, "denominator published, not null: {unit}");
}

/// Regression pin (GREEN today, must stay green): the flag is opt-in, so a mistyped `--repo`
/// still cannot sail past intake.
#[test]
fn plain_directory_without_the_flag_still_refuses_with_exit_3() {
    let repo = plain_dir();
    let (out, _) = run(repo.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(3),
        "env fault is exit 3: {}",
        why(&out)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not a git repository"),
        "message unchanged: {stderr}"
    );
}

/// `--no-git` on a real git worktree is legal and merely declines the merge -- so a wrapper that
/// always passes the flag never has to branch on whether the target happens to be a checkout.
#[test]
fn flag_on_a_real_git_worktree_is_legal_and_skips_merge() {
    let repo = git_dir();
    let (out, json) = run(repo.path(), &["--no-git"]);
    assert!(
        out.status.success(),
        "--no-git on a git repo is not an error: {}",
        why(&out)
    );
    let v = json.unwrap_or_else(|| panic!("--json must print one outcome: {}", why(&out)));
    assert_eq!(
        stage(&v, "Merge")["outcome"],
        "skip",
        "flag wins over detection: {v}"
    );
    assert_eq!(
        v["repo_mode"], "no-git",
        "declared mode, not detected mode: {v}"
    );
}
