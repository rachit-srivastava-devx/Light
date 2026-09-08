//! Per-cycle helpers for `worker::run_sink`: order validation and single-event retry delivery.

use fleet_types::Receipt;
use tokio::time::sleep;

use crate::cursor::CursorError;
use crate::event::StreamEvent;
use crate::log_source::LogSourceError;
use crate::sink::{Sink, SinkError};

use super::{PumpConfig, SinkStats};

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
pub(super) async fn deliver_with_retry(
    sink: &mut (dyn Sink + 'static),
    event: &StreamEvent,
    config: &PumpConfig,
    stats: &mut SinkStats,
) {
    let mut attempt = 0u32;
    loop {
        match sink.deliver(event) {
            Ok(()) => return stats.record_delivered(event.seq()),
            Err(SinkError::Permanent { .. }) => return stats.record_permanently_skipped(event.seq()),
            Err(SinkError::Transient { .. }) if attempt >= config.max_retries => {
                return stats.record_permanently_skipped(event.seq())
            }
            Err(SinkError::Transient { .. }) => {
                stats.record_retry();
                sleep(config.retry_backoff.delay(attempt)).await;
                attempt += 1;
            }
        }
    }
}
