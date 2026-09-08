use fleet_worker::{record_scorecard_outcome, scorecard_path, ScorecardOutcome};
use std::sync::Arc;
use std::thread;

/// D54b regression: concurrent `record_scorecard_outcome` calls for the same agent must not
/// lose an update -- the exclusive file lock must serialise the whole read-modify-write.
#[test]
fn concurrent_record_calls_never_lose_an_update() {
    let dir = tempfile::tempdir().unwrap();
    let state_dir = Arc::new(dir.path().to_path_buf());
    let threads = 16;
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let state_dir = Arc::clone(&state_dir);
            thread::spawn(move || {
                record_scorecard_outcome(&state_dir, "agent-x", ScorecardOutcome::Credited).unwrap();
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    let path = scorecard_path(&state_dir, "agent-x");
    let text = std::fs::read_to_string(path).unwrap();
    let card: fleet_worker::Scorecard = serde_json::from_str(&text).unwrap();
    assert_eq!(card.credited, threads);
    assert_eq!(card.checked, threads);
    assert_eq!(card.total, threads);
}
