use post::{verify_after_merge, GateResult, GateRunner, GateSpec, Grant, PostError, PostRequest, Status};
use std::{fs, path::Path};

struct CountingRunner { checked_per_gate: u64 }
impl GateRunner for CountingRunner {
    fn run(&self, _: &GateSpec, _: &Path) -> Result<GateResult, PostError> {
        Ok(GateResult { checked: self.checked_per_gate, total: self.checked_per_gate })
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

fn gate(id: &str) -> GateSpec {
    GateSpec { id: id.into() }
}

fn req(repo: &tempfile::TempDir, head: &str, gates: Vec<GateSpec>, digest: &str) -> PostRequest {
    PostRequest {
        repo: repo.path().to_path_buf(),
        expected_head: head.into(),
        acceptance_digest: digest.into(),
        required_gates: gates,
        grant: Grant { run_local_tests: true },
    }
}
#[test]
fn checked_sums_across_multiple_gates() {
    let head = "aabbccdd";
    let repo = make_repo(head);
    let runner = CountingRunner { checked_per_gate: 3 };
    let verdict = verify_after_merge(&runner, req(&repo, head, vec![gate("g1"), gate("g2")], "d")).unwrap();
    assert_eq!(verdict.checked, 6);
    assert_eq!(verdict.total, 2);
}
#[test]
fn evidence_digest_format_is_exact() {
    let head = "aabbccdd";
    let repo = make_repo(head);
    let runner = CountingRunner { checked_per_gate: 1 };
    let verdict = verify_after_merge(&runner, req(&repo, head, vec![gate("g1")], "mydigest")).unwrap();
    assert_eq!(verdict.evidence_digest, "mydigest:1/1");
}
#[test]
fn verdict_head_matches_repo_head() {
    let head = "deadbeef1234";
    let repo = make_repo(head);
    let runner = CountingRunner { checked_per_gate: 1 };
    let verdict = verify_after_merge(&runner, req(&repo, head, vec![gate("g1")], "d")).unwrap();
    assert_eq!(verdict.head, head);
    assert_eq!(verdict.status, Status::Pass);
}
#[test]
fn pass_receipt_file_has_correct_prefix() {
    let head = "aabbcc112233";
    let repo = make_repo(head);
    let runner = CountingRunner { checked_per_gate: 1 };
    verify_after_merge(&runner, req(&repo, head, vec![gate("g1")], "d")).unwrap();
    let store = repo.path().join(".fleet").join("post-receipts");
    let found = fs::read_dir(&store).unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().starts_with("pass-aabbcc"));
    assert!(found, "receipt must be named pass-<6 head chars>.json");
}
#[test]
fn single_gate_total_is_one() {
    let head = "cafe";
    let repo = make_repo(head);
    let runner = CountingRunner { checked_per_gate: 5 };
    let verdict = verify_after_merge(&runner, req(&repo, head, vec![gate("g1")], "d")).unwrap();
    assert_eq!(verdict.total, 1);
    assert_eq!(verdict.checked, 5);
}
