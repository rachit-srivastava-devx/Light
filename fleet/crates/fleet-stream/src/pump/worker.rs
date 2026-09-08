//! `run_sink`: poll/filter/deliver/retry/persist, one cycle at a time, for one sink.

use fleet_types::Receipt;
use tokio::sync::watch;
use tokio::time::sleep;

use crate::cursor::CursorStore;
use crate::event::StreamEvent;
use crate::log_source::{LogSource, LogSourceError};
use crate::sink::Sink;

use super::worker_cycle::{cursor_err, deliver_with_retry, validate_order};
use super::{PumpConfig, SinkStats};

/// # Panics
/// Never. Every fallible path returns inside `SinkStats`'s bookkeeping or ends the loop on a
/// fatal `LogSourceError`.
pub async fn run_sink(
    source: &(dyn LogSource + 'static),
    cursors: &(dyn CursorStore + 'static),
    sink: &mut (dyn Sink + 'static),
    config: &PumpConfig,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<SinkStats, LogSourceError> {
    let mut stats = SinkStats::default();
    loop {
        if *shutdown.borrow() {
            return Ok(stats);
        }
        let after = cursors.load(sink.id()).map_err(cursor_err)?;
        let receipts = source.poll_since(after)?;
        validate_order(&receipts, after)?;
        let accepted: Vec<Receipt> = receipts
            .into_iter()
            .filter(|receipt| sink.accepts(&StreamEvent(receipt.clone())))
            .take(config.queue_capacity)
            .collect();
        for receipt in &accepted {
            let event = StreamEvent(receipt.clone());
            deliver_with_retry(sink, &event, config, &mut stats).await;
            cursors.save(sink.id(), event.seq()).map_err(cursor_err)?;
        }
        if accepted.is_empty() {
            tokio::select! {
                _ = sleep(config.poll_interval) => {}
                _ = shutdown.changed() => {}
            }
        }
    }
}
