//! Per-cycle helpers for `worker::run_sink`: order validation and single-event retry delivery.

use backon::BackoffBuilder;
use tokio::time::sleep;
use types::Receipt;

use super::super::cursor::CursorError;
use super::super::event::StreamEvent;
use super::super::log_source::LogSourceError;
use super::super::sink::{Sink, SinkError};

use super::super::{PumpConfig, SinkStats};

pub(super) fn cursor_err(err: CursorError) -> LogSourceError {
    LogSourceError::Unavailable(format!("cursor store: {}", err.reason))
}

/// Every returned `seq` must be strictly greater than the previous one seen (starting at
/// `after`), matching `LogSource::poll_since`'s documented contract.
pub(super) fn validate_order(receipts: &[Receipt], after: Option<u64>) -> Result<(), LogSourceError> {
    let mut prev = after;
    for receipt in receipts {
        if let Some(bound) = prev {
            if receipt.seq <= bound {
                return Err(LogSourceError::OutOfOrder {
                    expected: bound,
                    got: receipt.seq,
                });
            }
        }
        prev = Some(receipt.seq);
    }
    Ok(())
}

/// Deliver one event, retrying `Transient` failures up to `max_retries` before downgrading to a
/// recorded, cursor-advancing skip -- never stalls this sink forever on one poison event.
///
/// Backoff timing is driven by [`backon::ExponentialBuilder`]; the retry loop itself is manual
/// because `&mut dyn Sink` cannot be captured by the `FnMut` async closure that backon's
/// `Retryable` trait requires (the returned future would borrow beyond the closure frame).
pub(super) async fn deliver_with_retry(
    sink: &mut (dyn Sink + 'static),
    event: &StreamEvent,
    config: &PumpConfig,
    stats: &mut SinkStats,
) {
    // Build a duration iterator: yields the sleep to take before each retry.
    // `with_max_times(n)` makes it yield at most n durations, so the iterator
    // exhausts after max_retries sleeps — matching the old `attempt >= max_retries` guard.
    let mut backoff = config.retry_backoff
        .clone()
        .with_max_times(config.max_retries as usize)
        .build();

    loop {
        match sink.deliver(event) {
            Ok(()) => return stats.record_delivered(event.seq()),
            Err(SinkError::Permanent { .. }) => return stats.record_permanently_skipped(event.seq()),
            Err(SinkError::Transient { .. }) => match backoff.next() {
                None => return stats.record_permanently_skipped(event.seq()),
                Some(dur) => {
                    stats.record_retry();
                    sleep(dur).await;
                }
            },
        }
    }
}
