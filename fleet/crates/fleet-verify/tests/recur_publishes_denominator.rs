//! Behavioural half of the stdout-denominator invariant (see `denominators_go_to_stdout.rs`):
//! actually RUN the gate that broke and confirm the registry's parser reads its stdout.
//! `recur-gate.sh` needs only `git` and `awk`, so this runs wherever the suite runs.

use fleet_verify::{DenominatorResult, GATES};
use std::path::{Path, PathBuf};
use std::process::Command;

fn gates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("gates")
}

fn run_recur(with_commit: bool) -> String {
    let repo = tempfile::tempdir().expect("tempdir");
    let git = |args: &[&str]| {
        let ok = Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .status()
            .expect("git must run")
            .success();
        assert!(ok, "git {args:?} failed");
    };
    git(&["init", "-q", "."]);
    if with_commit {
        std::fs::write(repo.path().join("a.txt"), "hello\n").expect("write");
        git(&["add", "-A"]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "init"]);
        std::fs::write(repo.path().join("a.txt"), "hello there\n").expect("write");
    }
    let out = Command::new("bash")
        .arg(gates_dir().join("recur-gate.sh"))
        .current_dir(repo.path())
        .output()
        .expect("recur-gate.sh must run");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Looked up by id in the committed registry, matching `parsers_table.rs` -- the parsers are
/// reviewed data, not this crate's public API.
fn recur_parser() -> fn(&str, &str) -> DenominatorResult {
    GATES
        .iter()
        .find(|g| g.id == "recur")
        .expect("the recur gate must be in the registry")
        .parse_denominator
}

/// Both cases matter: the empty repo took a prose-only early-exit that published nothing (a gate
/// passing on nothing, reported as an unreadable parse error), and the committed repo is the
/// path a real user hits.
#[test]
fn recur_publishes_a_parseable_denominator_on_stdout_alone() {
    let parser = recur_parser();
    for with_commit in [false, true] {
        let stdout = run_recur(with_commit);
        assert!(
            parser(&stdout, "") != DenominatorResult::Unparseable,
            "nothing parseable on stdout (with_commit={with_commit}); stdout was {stdout:?}"
        );
    }
}
