//! `CliAdapter` end-to-end -- no network/fs needed.

use fleet_events::{Adapter, Clock, CliAdapter};

struct FixedClock;
impl Clock for FixedClock {
    fn now_rfc3339(&self) -> String {
        "2026-01-01T00:00:00Z".to_string()
    }
}

#[test]
fn cli_adapter_produces_one_envelope_then_empties() {
    let mut adapter = CliAdapter::new(
        vec!["fleet".to_string(), "run".to_string()],
        "hello".to_string(),
        "nonce-1".to_string(),
    );

    let first = adapter.pull(&FixedClock).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].received_at, "2026-01-01T00:00:00Z");

    let second = adapter.pull(&FixedClock).unwrap();
    assert!(second.is_empty());

    let third = adapter.pull(&FixedClock).unwrap();
    assert!(third.is_empty());
}

#[test]
fn cli_adapter_ids_are_stable_across_reconstruction_with_same_nonce() {
    let mut a = CliAdapter::new(vec![], "".to_string(), "same-nonce".to_string());
    let mut b = CliAdapter::new(vec![], "".to_string(), "same-nonce".to_string());
    let ea = a.pull(&FixedClock).unwrap();
    let eb = b.pull(&FixedClock).unwrap();
    assert_eq!(ea[0].id, eb[0].id);
}
