//! The durable per-sink cursor: the highest seq a sink has successfully delivered.

/// Persisted by whatever `CursorStore` the caller injects (in production, `fleet-store`) --
/// this crate defines the shape, not the persistence mechanism.
pub trait CursorStore: Send + Sync {
    /// `Ok(None)` means this sink has never delivered anything -- a fresh sink starts from the
    /// beginning of the log, never from "now."
    fn load(&self, sink_id: &'static str) -> Result<Option<u64>, CursorError>;
    /// Persist `seq` as the new resume point. Must be durable before returning `Ok` -- a crash
    /// immediately after this call and before the next `deliver` must still resume correctly.
    fn save(&self, sink_id: &'static str, seq: u64) -> Result<(), CursorError>;
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("cursor store error for sink {sink_id:?}: {reason}")]
pub struct CursorError {
    pub sink_id: &'static str,
    pub reason: String,
}
