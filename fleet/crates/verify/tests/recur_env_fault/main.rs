//! Pins the fix for `classify()`'s exit-3 misclassification (AGENTS.md #7: exit 3 is the
//! project-wide "environment fault" contract, never an agent failure). `recur` is the concrete
//! repro: on a target repo with no `.git`, `recur-gate.sh`'s `git diff` calls fail before it can
//! even attempt its scan, so it exits 3 with `"recur-gate: git diff --cached failed"` -- distinct
//! from its exit-0-only `"recur-gate: not-applicable"` marker, so `parsers::recur` can't rescue
//! this case; the fix has to live in `classify()` itself.

use std::path::Path;
use std::process::Command;

use verify::{GatesRoot, ProbeTool, ProcessOutput, ProcessRunner, ToolProbe, Verdict, GATES};

struct AlwaysAvailable;
impl ToolProbe for AlwaysAvailable {
    fn available(&self, _tool: ProbeTool) -> bool {
        true
    }
}

/// Canned `ProcessOutput`, no spawn -- isolates `classify()` from the script's actual wording.
struct Canned(ProcessOutput);
impl ProcessRunner for Canned {
    fn run(&self, _command: &[&str]) -> ProcessOutput {
        self.0.clone()
    }
}

/// Real subprocess, `.current_dir(repo)` like `RealRunner`/`run_bounded` -- the script sees
/// `repo` as its cwd and (absent `FLEET_TARGET_REPO`) its own `$(pwd)` fallback.
struct RealShellRunner<'a> {
    repo: &'a Path,
}
impl ProcessRunner for RealShellRunner<'_> {
    fn run(&self, command: &[&str]) -> ProcessOutput {
        let (bin, rest) = command.split_first().expect("non-empty argv");
        let out = Command::new(bin)
            .args(rest)
            .current_dir(self.repo)
            .output()
            .unwrap_or_else(|e| panic!("failed to spawn {bin}: {e}"));
        ProcessOutput {
            exit_code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }
}

fn recur_spec() -> &'static verify::GateSpec {
    GATES
        .iter()
        .find(|g| g.id == "recur")
        .expect("recur gate registered")
}

fn assert_env_fault_skip(result: verify::GateResult) {
    match result.verdict {
        Verdict::Skip { was_required, ref reason } => {
            assert!(was_required, "recur is Required, so its skip must count in Report::env_faults(): {reason}");
        }
        other => panic!("expected Verdict::Skip (env fault, exit 3), got {other:?} -- not an invariant violation (AGENTS.md #7)"),
    }
}

#[test]
fn exit_3_is_skip_not_fail_for_a_synthetic_env_fault() {
    let spec = recur_spec();
    let gates = GatesRoot::materialize().expect("materialize embedded gate scripts");
    let runner = Canned(ProcessOutput {
        exit_code: 3,
        stdout: "recur-gate: repo /does/not/exist not accessible".to_string(),
        stderr: String::new(),
    });

    let result = verify::run_gate(spec, &AlwaysAvailable, &runner, &gates);
    assert_env_fault_skip(result);
}

#[test]
fn recur_reports_skip_not_fail_when_target_repo_has_no_git() {
    let plain_dir = tempfile::tempdir().expect("tempdir");
    // Sanity: genuinely not a git repo, or this test would pass downstream for the wrong reason.
    assert!(!plain_dir.path().join(".git").exists());

    let spec = recur_spec();
    let gates = GatesRoot::materialize().expect("materialize embedded gate scripts");
    let runner = RealShellRunner {
        repo: plain_dir.path(),
    };

    let result = verify::run_gate(spec, &AlwaysAvailable, &runner, &gates);
    assert_env_fault_skip(result);
}
