//! Durable verdict receipt writer — writes a JSON receipt for every verdict,
//! pass or fail, so the result is always auditable.
use crate::{PostError, PostVerdict, Status};
use std::{fs, path::Path};

/// Write `verdict` as a JSON file under `store_dir`.
///
/// Creates `store_dir` if it does not exist. Called for every verdict —
/// including failed ones — so that refusals remain auditable.
pub fn write_verdict(verdict: &PostVerdict, store_dir: &Path) -> Result<(), PostError> {
    fs::create_dir_all(store_dir).map_err(|e| PostError::Receipt(e.to_string()))?;
    let status_str = match verdict.status {
        Status::Pass => "pass",
        Status::Failed => "failed",
    };
    let prefix = verdict.head.get(..6).unwrap_or(&verdict.head);
    let name = format!("{}-{}.json", status_str, prefix);
    let path = store_dir.join(&name);
    let content = format!(
        "{{\"status\":\"{}\",\"head\":\"{}\",\"checked\":{},\"total\":{},\"evidence_digest\":\"{}\"}}",
        status_str, verdict.head, verdict.checked, verdict.total, verdict.evidence_digest
    );
    fs::write(&path, content).map_err(|e| PostError::Receipt(e.to_string()))
}
