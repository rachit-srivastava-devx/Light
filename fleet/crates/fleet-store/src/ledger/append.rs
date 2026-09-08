//! `append()`: verify-then-extend, blake3 hash, fs write.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::SystemTime;

use fleet_types::{Blake3Hash, ExitCode, PrevHash, Receipt, ReceiptEvent, SchemaV1};
use serde_json::Value;

use super::canon::canonical_bytes;
use super::clock::now_rfc3339;
use super::read::read_rows;
use super::types::LedgerError;
use super::verify::verify_rows;
use super::Ledger;
use crate::io_fault::IoFault;
use crate::lock::FileLock;

impl Ledger {
    /// Append one receipt. Verifies the existing chain first, refuses if it's already broken
    /// rather than extending a corrupt chain. Stamps `seq`/`prev_hash`/`ts_wall`/`hash` itself --
    /// the caller supplies only the fields a worker is allowed to author.
    pub fn append(
        &self,
        event: ReceiptEvent,
        body: Value,
        actor: String,
        resolved_model: Option<String>,
        exit_code: Option<ExitCode>,
    ) -> Result<Receipt, LedgerError> {
        let _guard = FileLock::acquire(&self.paths.lock)?;
        let rows = read_rows(&self.paths.chain)?;
        if !rows.is_empty() {
            verify_rows(&rows)?;
        }
        let seq = rows.len() as u64;
        let prev_hash = match rows.last() {
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
