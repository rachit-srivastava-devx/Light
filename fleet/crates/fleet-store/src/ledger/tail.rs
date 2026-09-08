//! `read_tail`: get just the last row (the chain's tip) in O(row size), not O(file size), so
//! `append` never has to load the whole chain to learn where to extend it.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use fleet_types::Receipt;

use super::canon::recompute_hash;
use super::types::LedgerError;
use crate::io_fault::IoFault;

const CHUNK: u64 = 8192;

/// The current on-disk tip, or `None` for an empty/never-initialized chain. Also re-checks that
/// tip row's own content hash, so appending never extends a tampered tip -- `verify()` remains
/// the only path that walks the whole chain.
pub(super) fn read_tail(path: &Path) -> Result<Option<Receipt>, LedgerError> {
    let Some(line) = last_line(path)? else { return Ok(None) };
    let receipt: Receipt = serde_json::from_str(&line)
        .map_err(|e| LedgerError::CorruptTail { reason: e.to_string() })?;
    let expected = recompute_hash(&receipt);
    if receipt.hash.as_str() != expected {
        return Err(LedgerError::Tampered { seq: receipt.seq });
    }
    Ok(Some(receipt))
}

/// Reads backward from EOF in chunks until two newlines (or the start of file) are in hand, then
/// returns the text of the last line. `Ok(None)` means the file is absent or empty -- a legitimate
/// "never appended to" state. Any other malformed tail (missing trailing newline from a crash
/// mid-write, or a blank trailing line) is `CorruptTail`, since we cannot assign it a `seq` until
/// it is parsed.
fn last_line(path: &Path) -> Result<Option<String>, LedgerError> {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(IoFault::Open { path: path.to_path_buf(), source }.into()),
    };
    let len = file.metadata().map_err(|source| IoFault::Read { path: path.to_path_buf(), source })?.len();
    if len == 0 {
        return Ok(None);
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut pos = len;
    loop {
        let take = CHUNK.min(pos);
        pos -= take;
        file.seek(SeekFrom::Start(pos)).map_err(|source| IoFault::Read { path: path.to_path_buf(), source })?;
        let mut chunk = vec![0u8; take as usize];
        file.read_exact(&mut chunk).map_err(|source| IoFault::Read { path: path.to_path_buf(), source })?;
        chunk.extend_from_slice(&buf);
        buf = chunk;
        if pos == 0 || buf.iter().filter(|&&b| b == b'\n').count() >= 2 {
            break;
        }
    }
    if buf.last() != Some(&b'\n') {
        return Err(LedgerError::CorruptTail {
            reason: "last row has no trailing newline (write interrupted mid-row)".into(),
        });
    }
    let content = &buf[..buf.len() - 1];
    let line = match content.iter().rposition(|&b| b == b'\n') {
        Some(idx) => &content[idx + 1..],
        None => content,
    };
    if line.is_empty() {
        return Err(LedgerError::CorruptTail { reason: "last row is a blank line".into() });
    }
    String::from_utf8(line.to_vec())
        .map(Some)
        .map_err(|e| LedgerError::CorruptTail { reason: e.to_string() })
}
