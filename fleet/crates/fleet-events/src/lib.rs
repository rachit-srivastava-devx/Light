//! The ingress plane: pull the outside world into one typed, guarded event envelope.
//!
//! This crate performs real IO (that is its entire purpose -- it is fleet's injection-surface
//! boundary), but every fact this crate does NOT itself need to touch the outside world for (the
//! wall clock, the durable log) arrives through an injected port (`Clock`, `EventSink`), so the
//! orchestration logic (`ingest_once`, `guard`) is testable without a network, a mailbox, or a
//! filesystem. `kind` is the only field anything downstream may use for control flow; `payload`
//! is untrusted data, never instructions, and `guard` is the one mandatory checkpoint before a
//! side-effecting `kind` becomes an actual action.

mod adapter;
mod adapters;
mod clock;
mod envelope;
mod event_kind;
mod guard;
mod ids;
mod ingest;
mod sink;
mod source_kind;

pub use adapter::Adapter;
pub use adapters::{
    CliAdapter, CliAdapterError, FsAdapter, FsAdapterError, GithubAdapter, GithubAdapterError,
    GmailAdapter, GmailAdapterError,
};
pub use clock::{Clock, SystemClock};
pub use envelope::{EventEnvelope, MAX_PAYLOAD_BYTES};
pub use event_kind::EventKind;
pub use guard::{guard, GuardedAction, MAX_QUOTE_BYTES};
pub use ids::EventId;
pub use ingest::{ingest_once, IngestError, IngestReport};
pub use sink::{EventSink, SinkError};
pub use source_kind::SourceKind;
