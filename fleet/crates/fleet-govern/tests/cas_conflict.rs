//! The direct regression test for the fan-out race this crate exists to close: N real OS
//! threads calling `admit` concurrently against one `FileMeterStore` backed by a `tempdir()`
//! file must never let the total granted exceed the lane's window.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use fleet_govern::{admit, FileMeterStore, LaneState, MeterStore};
use fleet_types::{LaneId, Tokens};

const THREADS: u64 = 16;
const PER_THREAD: u64 = 25;
// Deliberately half of what every thread combined would need: roughly half the admits must be
// refused with `InsufficientBudget`. Without `with_lane_locked`'s exclusive lock, two threads
// could both observe the same stale `remaining` and both be granted past it -- the fan-out race.
const WINDOW: u64 = (THREADS / 2) * PER_THREAD;

#[test]
fn concurrent_admits_never_oversubscribe_a_lane() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("meter-v1.tsv");
    let store = Arc::new(FileMeterStore::new(path));
    let lane = LaneId::parse("codex").unwrap();
    store
        .with_lane_locked(&lane, &mut |slot| {
            *slot = Some(LaneState {
                window: Some(Tokens::new(WINDOW)),
                used: Some(Tokens::ZERO),
                ..Default::default()
            })
        })
        .unwrap();

    let granted = Arc::new(AtomicU64::new(0));
    let handles: Vec<_> = (0..THREADS)
        .map(|_| {
            let store = Arc::clone(&store);
            let granted = Arc::clone(&granted);
            let lane = lane.clone();
            thread::spawn(move || {
                if admit(&*store, &lane, Tokens::new(PER_THREAD)).is_ok() {
                    granted.fetch_add(PER_THREAD, Ordering::SeqCst);
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }

    let mut final_state = None;
    store.with_lane_locked(&lane, &mut |slot| final_state = slot.clone()).unwrap();
    let final_state = final_state.unwrap();
    let persisted_used = final_state.used.unwrap().get();

    assert_eq!(persisted_used, granted.load(Ordering::SeqCst), "persisted `used` must equal the sum of every admit that returned Ok");
    assert!(persisted_used <= WINDOW, "total granted ({persisted_used}) exceeded window ({WINDOW})");
}
