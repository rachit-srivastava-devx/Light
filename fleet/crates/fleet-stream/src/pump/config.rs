//! Tuning for one sink's worker loop. Every sink gets its own `PumpConfig`.

use std::time::Duration;

#[derive(Clone, Debug)]
pub struct PumpConfig {
    /// How many receipts `poll_since` is allowed to hand this worker in one cycle before the
    /// rest wait for the next cycle -- the "bounded per-sink queue." Never drops a receipt: an
    /// over-the-cap remainder simply is not fetched this cycle, re-derived next cycle from the
    /// durable log.
    pub queue_capacity: usize,
    /// How long to sleep between poll cycles when nothing new was found.
    pub poll_interval: Duration,
    /// How many times to retry a `Transient` failure on the same event before treating it as if
    /// it were `Permanent` (logged, cursor still advances).
    pub max_retries: u32,
    /// Backoff between retries of the same event: `base * 2^attempt`, capped at `max`.
    pub retry_backoff: RetryBackoff,
}

#[derive(Clone, Copy, Debug)]
pub struct RetryBackoff {
    pub base: Duration,
    pub max: Duration,
}

impl RetryBackoff {
    /// `base * 2^attempt`, capped at `max`. `attempt` is 0-indexed (first retry is `attempt=0`).
    pub fn delay(&self, attempt: u32) -> Duration {
        self.base
            .checked_mul(1u32.checked_shl(attempt).unwrap_or(u32::MAX))
            .unwrap_or(self.max)
            .min(self.max)
    }
}

impl Default for PumpConfig {
    fn default() -> Self {
        Self {
            queue_capacity: 100,
            poll_interval: Duration::from_millis(200),
            max_retries: 3,
            retry_backoff: RetryBackoff {
                base: Duration::from_millis(50),
                max: Duration::from_secs(5),
            },
        }
    }
}
