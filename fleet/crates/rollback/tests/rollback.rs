use rollback::{
    rollback, ArtifactRecord, GitRollbackPort, Grant, RollbackError, RollbackReceipt,
    RollbackRequest, RollbackStore, RollbackTarget,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct MemStore { artifacts: HashMap<String, String>, pub receipts: Vec<String>, root: PathBuf }
impl MemStore {
    fn new(root: PathBuf) -> Self { Self { artifacts: HashMap::new(), receipts: Vec::new(), root } }
    fn insert(&mut self, id: &str, s: &str) { self.artifacts.insert(id.into(), s.into()); }
}
impl RollbackStore for MemStore {
    fn get_artifact(&self, id: &str) -> Option<ArtifactRecord> { self.artifacts.get(id).map(|s| ArtifactRecord { status: s.clone() }) }
    fn write_receipt(&mut self, r: &RollbackReceipt) { self.receipts.push(r.status.clone()); }
    fn worktrees_root(&self) -> &Path { &self.root }
}

struct FakeGit;
impl GitRollbackPort for FakeGit {
    fn remove_owned(&self, path: &Path) -> Result<(), RollbackError> {
        if path.is_dir() { std::fs::remove_dir_all(path).map_err(|e| RollbackError::NeedsReconciliation(e.to_string())) }
        else if path.exists() { std::fs::remove_file(path).map_err(|e| RollbackError::NeedsReconciliation(e.to_string())) }
        else { Ok(()) }
    }
    fn revert(&self, _: &Path, c: &str) -> Result<String, RollbackError> { Ok(c.to_string()) }
    fn head(&self, _: &Path) -> Result<String, RollbackError> { Ok(String::new()) }
}

fn req(id: &str, target: RollbackTarget) -> RollbackRequest {
    RollbackRequest { artifact_id: id.into(), target, reason: "test".into(), grant: Grant { allows: vec![] } }
}
#[test]
fn nonexistent_artifact_is_refused() {
    let mut store = MemStore::new(PathBuf::from("/fake/worktrees"));
    let result = rollback(&FakeGit, &mut store, req("no-such", RollbackTarget::OwnedWorktree(PathBuf::from("/fake/worktrees/wt"))));
    assert!(matches!(result, Err(RollbackError::ArtifactNotFound(_))));
    assert!(!store.receipts.is_empty());
}
#[test]
fn path_escape_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("worktrees");
    std::fs::create_dir_all(&root).unwrap();
    let mut store = MemStore::new(root);
    store.insert("art1", "pending");
    let result = rollback(&FakeGit, &mut store, req("art1", RollbackTarget::OwnedWorktree(PathBuf::from("../../sensitive"))));
    assert!(matches!(result, Err(RollbackError::Containment(_))));
    assert!(!store.receipts.is_empty());
}
#[test]
fn private_rollback_emits_receipt() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join(".worktrees");
    std::fs::create_dir_all(&root).unwrap();
    let wt = root.join("my-worktree");
    std::fs::create_dir(&wt).unwrap();
    let mut store = MemStore::new(root);
    store.insert("art1", "pending");
    let receipt = rollback(&FakeGit, &mut store, req("art1", RollbackTarget::OwnedWorktree(wt.clone()))).unwrap();
    assert_eq!(receipt.status, "rolled_back");
    assert_eq!(receipt.checked, 1);
    assert_eq!(receipt.total, 1);
    assert!(!wt.exists());
}
#[test]
fn rollback_emits_requeue_edge_on_shared_ref() {
    let mut store = MemStore::new(PathBuf::from("/fake/worktrees"));
    store.insert("art1", "pending");
    let result = rollback(&FakeGit, &mut store, req("art1", RollbackTarget::SharedRef {
        repo: PathBuf::from("/fake/repo"), merge_commit: "abc123".into(),
    }));
    assert!(matches!(result, Err(RollbackError::NeedsApproval(_))));
}
