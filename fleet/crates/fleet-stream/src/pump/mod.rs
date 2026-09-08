//! Pump orchestration: one worker loop per sink, tuned by a `PumpConfig`, reporting `SinkStats`.

mod config;
mod orchestrate;
mod stats;
mod worker;
mod worker_cycle;

pub use config::{PumpConfig, RetryBackoff};
pub use orchestrate::pump;
pub use stats::SinkStats;
pub use worker::run_sink;
