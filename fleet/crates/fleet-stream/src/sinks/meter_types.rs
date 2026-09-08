//! `MeterSink`'s reading types, split out to keep `meter.rs` under the line cap.

/// One receipt's honest usage reading. `labelled_cost_cents` is `None` unless the receipt body
/// itself named a cost -- never computed from `estimated_tokens * a guessed rate`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeterSample {
    pub seq: u64,
    pub estimated_tokens: u64,
    pub window_pct: Option<u8>,
    pub labelled_cost_cents: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct MeterSnapshot {
    pub samples: Vec<MeterSample>,
}
