use crate::StoreError;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Publish a blob atomically.
///
/// Rejects empty payloads (`IncompleteBlob`) and payloads whose SHA-256
/// does not match `ref_id` (`DigestMismatch`).  On success the blob is
/// written to `<dir>/<ref_id>` via a `.tmp` side-file and an atomic rename,
/// with an fsync before the rename so crash recovery is safe.
///
/// `ref_id` must be the lowercase hex-encoded SHA-256 of `payload`.
pub fn publish_blob(payload: &[u8], ref_id: &str, dir: &Path) -> Result<(), StoreError> {
    if payload.is_empty() {
        return Err(StoreError::IncompleteBlob);
    }
    if sha256_hex(payload) != ref_id {
        return Err(StoreError::DigestMismatch);
    }
    let tmp = dir.join(format!("{ref_id}.tmp"));
    let dst = dir.join(ref_id);
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| StoreError::Io(e.to_string()))?;
        f.write_all(payload)
            .map_err(|e| StoreError::Io(e.to_string()))?;
        f.sync_all().map_err(|e| StoreError::Io(e.to_string()))?;
    }
    std::fs::rename(&tmp, &dst).map_err(|e| StoreError::Io(e.to_string()))?;
    Ok(())
}
