use crate::grant::{ApprovalError, ApprovalGrant};

/// Persistence port for approval records.
///
/// Implementations MUST be durable: `put` must write before returning `Ok`,
/// and `consume` must atomically mark the grant consumed so a second call
/// returns `Err(ApprovalError::Replay)`.
pub trait ApprovalStore {
    /// Durably record a new grant. Returns `Err(Store(_))` on write failure.
    fn put(&mut self, grant: &ApprovalGrant) -> Result<(), ApprovalError>;

    /// Atomically mark the grant identified by `id` consumed and return it.
    /// Returns `Err(Replay)` if already consumed, `Err(NotFound(_))` if unknown.
    fn consume(&mut self, id: &str) -> Result<ApprovalGrant, ApprovalError>;
}
