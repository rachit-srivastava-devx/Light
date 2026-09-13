use std::path::PathBuf;

pub enum RollbackTarget {
    OwnedWorktree(PathBuf),
    PrivateRef {
        repo: PathBuf,
        before: String,
        applied: String,
    },
    SharedRef {
        repo: PathBuf,
        merge_commit: String,
    },
}

pub struct Grant {
    pub allows: Vec<String>,
}
pub struct ArtifactRecord {
    pub status: String,
}
pub struct RollbackRequest {
    pub artifact_id: String,
    pub target: RollbackTarget,
    pub reason: String,
    pub grant: Grant,
}
pub struct RollbackReceipt {
    pub artifact_id: String,
    pub status: String,
    pub before: String,
    pub after: String,
    pub checked: u64,
    pub total: u64,
}
