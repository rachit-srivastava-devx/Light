use crate::types::{DagError, GraphVersion};

/// Port trait for CAS version persistence. Single-writer through the store.
pub trait VersionStore: Send + Sync {
    /// Verify that the given revision is newer than the stored one.
    fn check_revision(&self, version: &GraphVersion) -> Result<(), DagError>;
    /// Persist the version (caller must call check_revision first).
    fn store(&mut self, version: &GraphVersion) -> Result<(), DagError>;
}

/// In-memory VersionStore for tests and composition roots without a durable backend.
#[derive(Default)]
pub struct MemoryVersionStore {
    revision: Option<u64>,
}

impl VersionStore for MemoryVersionStore {
    fn check_revision(&self, version: &GraphVersion) -> Result<(), DagError> {
        if let Some(stored) = self.revision {
            if version.revision <= stored {
                return Err(DagError::StaleRevision);
            }
        }
        Ok(())
    }

    fn store(&mut self, version: &GraphVersion) -> Result<(), DagError> {
        self.revision = Some(version.revision);
        Ok(())
    }
}
