//! `LedgerLogSource`: adapts `fleet_store::Ledger` to `fleet_stream::LogSource` so the egress
//! side can tail the exact same durable ledger `event_stage`/`verify_stage`/`run_ledger` append
//! to -- one durable log, one durable-write path (`Ledger::append`), one durable-read path
//! (here), never a second parallel event store invented for streaming alone.

use fleet_store::Ledger;
use fleet_stream::{LogSource, LogSourceError};
use fleet_types::Receipt;

pub struct LedgerLogSource {
    ledger: Ledger,
}

impl LedgerLogSource {
    pub fn new(ledger: Ledger) -> Self {
        Self { ledger }
    }
}

impl LogSource for LedgerLogSource {
    /// `Ledger::rows` re-reads the whole chain every call -- fine at this crate's scale (one CLI
    /// run's worth of receipts), and it means this adapter carries no state of its own to ever
    /// drift from what `Ledger::verify` would say is really on disk.
    fn poll_since(&self, after: Option<u64>) -> Result<Vec<Receipt>, LogSourceError> {
        let rows = self.ledger.rows(true).map_err(|e| LogSourceError::Unavailable(e.to_string()))?;
        Ok(rows.into_iter().filter(|r| after.map(|a| r.seq > a).unwrap_or(true)).collect())
    }
}
