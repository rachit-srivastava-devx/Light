use crate::{RollbackError, RollbackReceipt};
use std::path::Path;

pub(crate) fn verify_removal(path: &Path) -> Result<String, RollbackError> {
    if path.exists() {
        Err(RollbackError::NeedsReconciliation(format!(
            "path still exists after removal: {}",
            path.display()
        )))
    } else {
        Ok(String::new())
    }
}

pub fn verify_action(
    result: &Result<RollbackReceipt, RollbackError>,
    expected_status: &str,
) -> bool {
    match result {
        Ok(r) => r.status == expected_status,
        Err(_) => false,
    }
}
