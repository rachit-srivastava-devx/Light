use crate::RollbackReceipt;

pub(crate) fn refuse(artifact_id: &str) -> RollbackReceipt {
    RollbackReceipt {
        artifact_id: artifact_id.to_string(),
        status: "refused".to_string(),
        before: String::new(),
        after: String::new(),
        checked: 0,
        total: 1,
    }
}

pub(crate) fn success(artifact_id: &str, before: &str, after: &str) -> RollbackReceipt {
    RollbackReceipt {
        artifact_id: artifact_id.to_string(),
        status: "rolled_back".to_string(),
        before: before.to_string(),
        after: after.to_string(),
        checked: 1,
        total: 1,
    }
}
