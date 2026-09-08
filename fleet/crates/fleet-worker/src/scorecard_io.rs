//! File-locked (RMW) scorecard persistence, ported verbatim from `agent.rs`'s
//! `record_scorecard_outcome` -- fixes D54b's lost-update race under concurrent dispatch.

use crate::scorecard::{Scorecard, ScorecardOutcome};
use fs2::FileExt;
use std::fs;
use std::path::{Path, PathBuf};

pub fn scorecard_path(state_dir: &Path, agent_id: &str) -> PathBuf {
    state_dir.join("scorecards").join(format!("{agent_id}.json"))
}

/// Record one attributable outcome for an agent, serialised by an exclusive file lock across
/// the whole read-modify-write -- D54b's fix, ported verbatim.
pub fn record_scorecard_outcome(
    state_dir: &Path,
    agent_id: &str,
    outcome: ScorecardOutcome,
) -> Result<Scorecard, String> {
    let path = scorecard_path(state_dir, agent_id);
    fs::create_dir_all(path.parent().ok_or("no parent dir")?).map_err(|e| e.to_string())?;
    let lock_path = path.with_extension("lock");
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|e| e.to_string())?;
    lock.lock_exclusive().map_err(|e| e.to_string())?;
    let result = (|| {
        let mut scorecard = match fs::read_to_string(&path) {
            Ok(text) => {
                let card: Scorecard = serde_json::from_str(&text).map_err(|e| e.to_string())?;
                if !card.consistent() {
                    return Err(format!("corrupt scorecard at {}", path.display()));
                }
                card
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Scorecard::empty(agent_id),
            Err(e) => return Err(e.to_string()),
        };
        scorecard.apply(outcome);
        let bytes = serde_json::to_vec_pretty(&scorecard).map_err(|e| e.to_string())?;
        fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(scorecard)
    })();
    let _ = FileExt::unlock(&lock);
    result
}
