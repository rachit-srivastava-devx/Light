//! `LoopStore` -- the injected persistence port for `LoopProgress`. The entire "resume days
//! later" guarantee lives here: `AutonomousRun` never keeps progress in memory across a restart,
//! it re-`load`s it every `tick`. `FileLoopStore` (`loop_store_file.rs`) is the real
//! implementation; tests use an in-memory fake instead of the repo tree or `$HOME`.

use crate::loop_types::{LoopIoError, LoopProgress};

pub trait LoopStore: Send + Sync {
    /// `Ok(None)` means no run has ever recorded progress for `plan_id` -- start from unit 0,
    /// never an error.
    fn load(&self, plan_id: &str) -> Result<Option<LoopProgress>, LoopIoError>;

    /// Durably replace `plan_id`'s progress with `progress`. Must be atomic from a reader's point
    /// of view (a crash mid-publish must never leave a half-written, unparsable file behind).
    fn save(&self, plan_id: &str, progress: &LoopProgress) -> Result<(), LoopIoError>;
}
