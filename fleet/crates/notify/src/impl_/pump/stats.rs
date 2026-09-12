//! Per-sink outcome counters, returned when a sink's worker loop stops.

/// Not a substitute for the sink's own tests -- the minimum an operator needs to know whether a
/// sink is keeping up.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SinkStats {
    pub delivered: u64,
    pub retried: u64,
    pub permanently_skipped: u64,
    pub last_delivered_seq: Option<u64>,
}

impl SinkStats {
    pub(super) fn record_delivered(&mut self, seq: u64) {
        self.delivered += 1;
        self.last_delivered_seq = Some(seq);
    }

    pub(super) fn record_permanently_skipped(&mut self, seq: u64) {
        self.permanently_skipped += 1;
        self.last_delivered_seq = Some(seq);
    }

    pub(super) fn record_retry(&mut self) {
        self.retried += 1;
    }
}
