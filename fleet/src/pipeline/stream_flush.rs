//! Optional egress flush: when `FLEET_STREAM_DIR` is set, tail the run's ledger and hand every
//! new receipt to a durable `FileSink`, cursored by `FileCursorStore` so a later run resumes
//! instead of replaying. Unset (the default), this module never touches the filesystem at all --
//! behaviour is byte-identical to before this wiring existed. A delivery failure here is reported
//! (the repo's most-repeated defect is a swallowed failure) but never fails the pipeline run --
//! observability must not be able to break the product it observes.

use super::ledger_events;
use super::ledger_log_source::LedgerLogSource;
use crate::print::human_stream::emit;
use crate::print::render_event::Event;
use crate::print::style::Style;
use fleet_stream::sinks::FileSink;
use fleet_stream::{CursorStore, FileCursorStore, LogSource, Sink, SinkError, StreamEvent};
use std::path::Path;

pub const FLEET_STREAM_DIR: &str = "FLEET_STREAM_DIR";

/// The one call site `dispatch::run_cmd` invokes after every pipeline run. Reads the env var
/// itself so every caller gets the same off-by-default gate for free.
pub fn maybe_flush(state_dir: &Path) {
    let Ok(dir) = std::env::var(FLEET_STREAM_DIR) else { return };
    if let Err(reason) = flush(state_dir, Path::new(&dir)) {
        // Name the directory AND the env var that named it: the failure used to surface as a bare
        // `No such file or directory (os error 2)` with no path in it, which reads as noise rather
        // than as "your FLEET_STREAM_DIR could not be written".
        let text = format!("{FLEET_STREAM_DIR}={dir}: {reason}");
        emit(&Event::Note { source: "fleet-stream".to_string(), text }, &Style::detect());
    }
}

fn flush(state_dir: &Path, stream_dir: &Path) -> Result<(), String> {
    // `FileSink` opens its NDJSON file with `create(true)`, which creates the FILE but never its
    // parent DIRECTORY -- so a `FLEET_STREAM_DIR` that did not already exist produced no file, no
    // cursor, and (before the message above) an unattributed errno. The env var names a directory
    // fleet is being told to write; creating it (and its parents) is this function's job.
    std::fs::create_dir_all(stream_dir)
        .map_err(|e| format!("could not create stream directory: {e}"))?;
    let source = LedgerLogSource::new(ledger_events::open(state_dir));
    let cursors = FileCursorStore::new(stream_dir.join("cursors"));
    let mut sink = FileSink::new(stream_dir.join("events.ndjson"), vec![]);
    let after = cursors.load(sink.id()).map_err(|e| e.reason)?;
    let receipts = source.poll_since(after).map_err(|e| e.to_string())?;
    for receipt in receipts {
        let event = StreamEvent(receipt);
        if !sink.accepts(&event) {
            continue;
        }
        match sink.deliver(&event) {
            // Matches `fleet_stream::pump`'s own semantics: a permanent per-event rejection
            // still advances the cursor (this one event is never retried); a transient failure
            // does not, so the NEXT flush (next `fleet run`, or a re-run of this one) retries it.
            Ok(()) => cursors.save(sink.id(), event.seq()).map_err(|e| e.reason)?,
            Err(SinkError::Permanent { reason, .. }) => {
                cursors.save(sink.id(), event.seq()).map_err(|e| e.reason)?;
                return Err(reason);
            }
            Err(SinkError::Transient { reason, .. }) => return Err(reason),
        }
    }
    Ok(())
}
