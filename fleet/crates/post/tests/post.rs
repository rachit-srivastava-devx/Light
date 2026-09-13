use post::{
    verify_after_merge, GateResult, GateRunner, GateSpec, Grant, PostError, PostRequest, Status,
};
use std::{fs, path::Path};

struct FakeRunner {
    fail: bool,
}
impl GateRunner for FakeRunner {
    fn run(&self, _: &GateSpec, _: &Path) -> Result<GateResult, PostError> {
        if self.fail {
            Err(PostError::GateFailed)
        } else {
            Ok(GateResult {
                checked: 1,
                total: 1,
            })
        }
    }
}

fn make_repo(head: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let git = dir.path().join(".git");
    fs::create_dir_all(git.join("refs/heads")).unwrap();
    fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(git.join("refs/heads/main"), format!("{}\n", head)).unwrap();
    dir
}

fn req(repo: &tempfile::TempDir, expected_head: &str, gates: Vec<GateSpec>) -> PostRequest {
    PostRequest {
        repo: repo.path().to_path_buf(),
        expected_head: expected_head.into(),
        acceptance_digest: String::new(),
        required_gates: gates,
        grant: Grant {
            run_local_tests: true,
        },
    }
}

fn one_gate() -> Vec<GateSpec> {
    vec![GateSpec { id: "g1".into() }]
}
#[test]
fn stale_head_refuses() {
    let repo = make_repo("def456");
    let result = verify_after_merge(
        &FakeRunner { fail: false },
        req(&repo, "abc123", one_gate()),
    );
    assert!(
        matches!(result, Err(PostError::Stale)),
        "expected Stale, got {result:?}"
    );
}
#[test]
fn zero_gate_refuses() {
    let repo = make_repo("abc123");
    let result = verify_after_merge(&FakeRunner { fail: false }, req(&repo, "abc123", vec![]));
    assert!(
        matches!(result, Err(PostError::ZeroCoverage)),
        "expected ZeroCoverage, got {result:?}"
    );
}
#[test]
fn refusal_still_writes_receipt() {
    let repo = make_repo("abc123");
    let verdict = verify_after_merge(&FakeRunner { fail: true }, req(&repo, "abc123", one_gate()))
        .expect("gate failure should return Ok with Failed status, not Err");
    assert_eq!(verdict.status, Status::Failed, "verdict must be Failed");
    let store = repo.path().join(".fleet").join("post-receipts");
    let entries: Vec<_> = fs::read_dir(&store)
        .expect("post-receipts dir must exist")
        .collect();
    assert!(
        !entries.is_empty(),
        "receipt must be written even on failure"
    );
}
