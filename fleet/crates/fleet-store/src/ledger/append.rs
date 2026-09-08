//! `append()`: verify-then-extend, blake3 hash, fs write.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::SystemTime;

use fleet_types::{Blake3Hash, ExitCode, PrevHash, Receipt, ReceiptEvent, SchemaV1};
use serde_json::Value;

use super::canon::canonical_bytes;
use super::clock::now_rfc3339;
use super::tail::read_tail;
use super::types::LedgerError;
use super::Ledger;
use crate::io_fault::IoFault;
use crate::lock::FileLock;

impl Ledger {
    /// Append one receipt. O(1) amortised in chain length: reads only the current on-disk tip
    /// (not the whole chain) to learn `seq`/`prev_hash`, and refuses rather than extending a tip
    /// that is unreadable or whose hash does not recompute. The file lock is held across that
    /// read and the write below, so a concurrent process's append is never raced onto a stale
    /// tip. Full-chain corruption elsewhere is still caught -- just by `verify()`, not by every
    /// `append()`. Stamps `seq`/`prev_hash`/`ts_wall`/`hash` itself -- the caller supplies only
    /// the fields a worker is allowed to author.
    pub fn append(
        &self,
        event: ReceiptEvent,
        body: Value,
        actor: String,
        resolved_model: Option<String>,
        exit_code: Option<ExitCode>,
    ) -> Result<Receipt, LedgerError> {
        let _guard = FileLock::acquire(&self.paths.lock)?;
        let tip = read_tail(&self.paths.chain)?;
        let seq = tip.as_ref().map_or(0, |r| r.seq + 1);
        let prev_hash = match &tip {
            Some(last) => PrevHash::Hash(last.hash.clone()),
            None => PrevHash::Genesis,
        };
        let ts_wall = now_rfc3339(SystemTime::now());
        let canonical = canonical_bytes(
            &SchemaV1, seq, &prev_hash, &ts_wall, &event, &actor, &resolved_model, &exit_code, &body,
        );
        let mut hash_input = prev_hash.as_str().as_bytes().to_vec();
        hash_input.extend_from_slice(&canonical);
        let hash_hex = blake3::hash(&hash_input).to_hex().to_string();
        let hash = Blake3Hash::parse(format!("blake3:{hash_hex}"))
            .expect("a freshly blake3-hashed digest always matches the wire shape");
        let receipt = Receipt {
            schema_version: SchemaV1,
            seq,
            prev_hash,
            hash,
            ts_wall,
            event,
            actor,
            resolved_model,
            exit_code,
            body,
        };
        let line = serde_json::to_string(&receipt)
            .map_err(|e| LedgerError::MalformedRow { seq, reason: e.to_string() })?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.paths.chain)
            .map_err(|source| IoFault::Open { path: self.paths.chain.clone(), source })?;
        file.write_all(line.as_bytes())
            .map_err(|source| IoFault::Write { path: self.paths.chain.clone(), source })?;
        file.write_all(b"\n")
            .map_err(|source| IoFault::Write { path: self.paths.chain.clone(), source })?;
        file.sync_data()
            .map_err(|source| IoFault::Write { path: self.paths.chain.clone(), source })?;
        Ok(receipt)
    }
}
