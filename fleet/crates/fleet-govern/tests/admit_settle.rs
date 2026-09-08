//! Single-threaded `admit`/`settle` behavior-spec cases (§9) against an `InMemoryStore` this test
//! file owns. The real-file round trip and the publish-failure case live in
//! `admit_settle_file.rs` (kept out of this file to stay under the 80-line-per-file rule).

mod support;

use fleet_govern::{admit, settle, AdmitError, LaneState, SettleError};
use fleet_types::Tokens;
use support::{lane, measured, InMemoryStore};

#[test]
fn admit_refuses_unknown_lane() {
    let store = InMemoryStore::new();
    let err = admit(&store, &lane("codex"), Tokens::new(1)).unwrap_err();
    assert!(matches!(err, AdmitError::UnknownLane(l) if l == "codex"));
}

#[test]
fn admit_refuses_unmeasured_window_and_used() {
    let store = InMemoryStore::new();
    store.seed(&lane("codex"), LaneState { used: Some(Tokens::ZERO), ..Default::default() });
    let err = admit(&store, &lane("codex"), Tokens::new(1)).unwrap_err();
    assert!(matches!(err, AdmitError::WindowUnknown(_)));

    let store = InMemoryStore::new();
    store.seed(&lane("codex"), LaneState { window: Some(Tokens::new(10)), ..Default::default() });
    let err = admit(&store, &lane("codex"), Tokens::new(1)).unwrap_err();
    assert!(matches!(err, AdmitError::UsedUnknown(_)));
}

#[test]
fn admit_boundary_exact_fit_succeeds_one_over_refuses() {
    let store = InMemoryStore::new();
    store.seed(&lane("codex"), measured(500, 0));
    assert!(admit(&store, &lane("codex"), Tokens::new(500)).is_ok());

    let store = InMemoryStore::new();
    store.seed(&lane("codex"), measured(500, 0));
    let err = admit(&store, &lane("codex"), Tokens::new(501)).unwrap_err();
    assert!(matches!(err, AdmitError::InsufficientBudget { .. }));
}

#[test]
fn settle_unknown_reservation_id_is_refused() {
    let store = InMemoryStore::new();
    store.seed(&lane("codex"), measured(500, 0));
    store.seed(&lane("claude"), measured(500, 0));
    let r = admit(&store, &lane("codex"), Tokens::new(10)).unwrap();
    // Same id value, wrong lane -- not present in "claude"'s reservations, so unknown there.
    let mismatched = fleet_govern::Reservation { lane: lane("claude"), ..r };
    let err = settle(&store, mismatched, Tokens::new(0)).unwrap_err();
    assert!(matches!(err, SettleError::UnknownReservation(_)));
}

#[test]
fn settle_twice_on_same_reservation_is_refused() {
    let store = InMemoryStore::new();
    store.seed(&lane("codex"), measured(500, 0));
    let r = admit(&store, &lane("codex"), Tokens::new(10)).unwrap();
    settle(&store, r.clone(), Tokens::new(5)).unwrap();
    let err = settle(&store, r, Tokens::new(5)).unwrap_err();
    assert!(matches!(err, SettleError::UnknownReservation(_)));
}
