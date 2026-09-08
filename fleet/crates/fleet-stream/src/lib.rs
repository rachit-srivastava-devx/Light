//! The egress plane: tail the append-only receipt log, project each row, fan out to pluggable
//! sinks. The mirror of `fleet-events` on the ingress side.
//!
//! This crate never opens the ledger file itself -- every fact about "what's new" arrives
//! through `LogSource`, injected by the caller (in production, `fleet-store`). Each `Sink` is
//! delivered to independently: one sink stalling (network down, disk full) never slows or drops
//! events for any other sink, because each sink owns its own durable cursor and its own bounded
//! read-ahead queue.

mod cursor;
mod cursor_file;
mod event;
mod log_source;
mod pump;
mod sink;
pub mod sinks;

pub use cursor::{CursorError, CursorStore};
pub use cursor_file::FileCursorStore;
pub use event::StreamEvent;
pub use log_source::{LogSource, LogSourceError};
pub use pump::{pump, run_sink, PumpConfig, RetryBackoff, SinkStats};
pub use sink::{Sink, SinkError};
