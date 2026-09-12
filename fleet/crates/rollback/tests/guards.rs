use rollback::{rollback, verify_action, ArtifactRecord, GitRollbackPort, Grant, RollbackError,
    RollbackReceipt, RollbackRequest, RollbackStore, RollbackTarget};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct Mem { arts: HashMap<String, String>, receipts: Vec<String>, root: PathBuf }
impl Mem {
    fn seed(root: PathBuf, id: &str, s: &str) -> Self { let mut m = Self { arts: HashMap::new(), receipts: vec![], root }; m.arts.insert(id.into(), s.into()); m }
}
impl RollbackStore for Mem {
    fn get_artifact(&self, id: &str) -> Option<ArtifactRecord> { self.arts.get(id).map(|s| ArtifactRecord { status: s.clone() }) }
    fn write_receipt(&mut self, r: &RollbackReceipt) { self.receipts.push(r.status.clone()); }
    fn worktrees_root(&self) -> &Path { &self.root }
}
struct Git(String);
impl GitRollbackPort for Git {
    fn remove_owned(&self, _: &Path) -> Result<(), RollbackError> { Ok(()) }
    fn revert(&self, _: &Path, c: &str) -> Result<String, RollbackError> { Ok(c.into()) }
    fn head(&self, _: &Path) -> Result<String, RollbackError> { Ok(self.0.clone()) }
}
fn req(id: &str, t: RollbackTarget) -> RollbackRequest {
    RollbackRequest { artifact_id: id.into(), target: t, reason: "t".into(), grant: Grant { allows: vec![] } }
}
#[test]
fn already_rolled_back_is_refused() {
    let mut s = Mem::seed("/r".into(), "a1", "rolled_back");
    let r = rollback(&Git("h".into()), &mut s, req("a1", RollbackTarget::SharedRef {
        repo: "/repo".into(), merge_commit: "m".into() }));
    assert!(matches!(r, Err(RollbackError::Receipt(_))));
    assert_eq!(s.receipts, vec!["refused"]);
}
#[test]
fn cas_mismatch_is_refused() {
    let mut s = Mem::seed("/r".into(), "a1", "pending");
    let r = rollback(&Git("HEAD_HASH".into()), &mut s, req("a1", RollbackTarget::PrivateRef {
        repo: "/repo".into(), before: "b".into(), applied: "WRONG".into() }));
    assert!(matches!(r, Err(RollbackError::CasMismatch(_))));
    assert_eq!(s.receipts, vec!["refused"]);
}
#[test]
fn cas_match_private_ref_succeeds() {
    let mut s = Mem::seed("/r".into(), "a1", "pending");
    let r = rollback(&Git("abc".into()), &mut s, req("a1", RollbackTarget::PrivateRef {
        repo: "/nonexistent_xyz".into(), before: "b".into(), applied: "abc".into() }));
    assert!(matches!(r, Ok(_)));
    assert_eq!(s.receipts, vec!["rolled_back"]);
}
#[test]
fn containment_dotdot_inside_root_is_accepted() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap().join("wt");
    std::fs::create_dir(&root).unwrap();
    let target = root.join("sub").join("..").join("missing");
    let mut s = Mem::seed(root, "a1", "pending");
    let r = rollback(&Git("h".into()), &mut s, req("a1", RollbackTarget::OwnedWorktree(target)));
    let receipt = r.expect("dotdot inside root must succeed");
    assert_eq!(receipt.status, "rolled_back");
    assert_eq!(s.receipts, vec!["rolled_back"]);
}
#[test]
fn verify_removal_detects_path_still_exists() {
    let tmp = tempfile::tempdir().unwrap();
    let wt = tmp.path().join("wt");
    std::fs::create_dir(&wt).unwrap();
    let mut s = Mem::seed(tmp.path().into(), "a1", "pending");
    let r = rollback(&Git("h".into()), &mut s, req("a1", RollbackTarget::OwnedWorktree(wt)));
    assert!(matches!(r, Err(RollbackError::NeedsReconciliation(_))));
    assert_eq!(s.receipts, vec!["refused"]);
}
#[test]
fn verify_action_true_for_ok_correct_status() {
    let ok: Result<RollbackReceipt, RollbackError> = Ok(RollbackReceipt {
        artifact_id: "a".into(), status: "rolled_back".into(), before: "".into(), after: "".into(), checked: 1, total: 1 });
    assert!(verify_action(&ok, "rolled_back")); assert!(!verify_action(&ok, "refused"));
}
#[test]
fn verify_action_false_for_err() {
    let e: Result<RollbackReceipt, RollbackError> = Err(RollbackError::ArtifactNotFound("x".into()));
    assert!(!verify_action(&e, "rolled_back"));
}
