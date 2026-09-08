//! The real-file `admit`/`settle` round trip and the publish-failure case (never the repo tree
//! or `$HOME` -- this uses `tempfile::tempdir()`). Split out of `admit_settle.rs` to stay under
//! the 80-line-per-file rule.

mod support;

use std::sync::atomic::Ordering;

use fleet_govern::{admit, settle, AdmitError, FileMeterStore, MeterStore};
use fleet_types::Tokens;
use support::{lane, measured, FailingPublishStore, InMemoryStore};

#[test]
fn admit_then_settle_round_trips_through_a_real_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("meter-v1.tsv");
    let store = FileMeterStore::new(path.clone());
    store.with_lane_locked(&lane("codex"), &mut |slot| *slot = Some(measured(1000, 0))).unwrap();

    let reservation = admit(&store, &lane("codex"), Tokens::new(100)).unwrap();
    settle(&store, reservation, Tokens::new(42)).unwrap();

    let reopened = FileMeterStore::new(path);
    let mut seen = None;
    reopened.with_lane_locked(&lane("codex"), &mut |slot| seen = slot.clone()).unwrap();
    assert_eq!(seen.unwrap().used, Some(Tokens::new(42)));
}

#[test]
fn admit_publish_failure_leaves_no_partial_reservation() {
    let store = FailingPublishStore { inner: InMemoryStore::new(), fail: true.into() };
    store.inner.seed(&lane("codex"), measured(500, 0));

    let err = admit(&store, &lane("codex"), Tokens::new(100)).unwrap_err();
    assert!(matches!(err, AdmitError::Store(_)));

    store.fail.store(false, Ordering::SeqCst);
    let reservation = admit(&store, &lane("codex"), Tokens::new(500)).unwrap();
    assert_eq!(reservation.estimated, Tokens::new(500));
}
