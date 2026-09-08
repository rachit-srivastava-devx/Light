//! Shared test helpers for `fleet-stream`'s own test binaries.
#![allow(dead_code)]

use std::time::Duration;

use fleet_stream::{PumpConfig, RetryBackoff};

/// A `PumpConfig` tuned for fast, deterministic crash/resume tests: short poll interval, short
/// retry backoff, so a test does not need multi-second sleeps to observe a pump cycle.
pub fn fast_config() -> PumpConfig {
    PumpConfig {
        queue_capacity: 100,
        poll_interval: Duration::from_millis(5),
        max_retries: 3,
        retry_backoff: RetryBackoff { base: Duration::from_millis(1), max: Duration::from_millis(5) },
    }
}
