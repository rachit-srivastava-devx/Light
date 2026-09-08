//! `StreamEvent` -- the egress plane's own thin wrapper around a ledger `Receipt`.

use fleet_types::Receipt;

/// One ledger receipt as it flows through the egress pump. A thin wrapper (not a bare alias) so
/// this crate can add pump-internal bookkeeping later without changing `fleet_types::Receipt`.
#[derive(Clone, Debug)]
pub struct StreamEvent(pub Receipt);

impl StreamEvent {
    /// The ledger's own monotonic sequence number -- the sole dedupe/ordering key every sink
    /// cursor is keyed on (never a wall-clock timestamp, never an insertion-order guess).
    pub fn seq(&self) -> u64 {
        self.0.seq
    }
}
