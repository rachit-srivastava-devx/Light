//! Regression pin (adversarial-verifier finding, 2026-09-08): `settle` must reject a stale/duplicate
//! `ReservationId` even when the lane still has ANOTHER open reservation. The pre-existing
//! cross-lane test left the target lane's reservation list empty, so `.position(...)` returned
//! `None` regardless of whether the id comparison actually ran — neutering it to `|_| true` passed
//! the whole suite. This test keeps a second reservation open so a neutered id check would wrongly
//! settle it, and asserts the real behavior: the stale id is refused, not silently applied.

mod support;
use support::*;

use fleet_govern::{admit, settle, SettleError};
use fleet_types::Tokens;

#[test]
fn settle_stale_id_does_not_settle_a_different_open_reservation() {
    let store = InMemoryStore::new();
    store.seed(&lane("codex"), measured(500, 0));

    let r1 = admit(&store, &lane("codex"), Tokens::new(10)).unwrap();
    let _r2 = admit(&store, &lane("codex"), Tokens::new(20)).unwrap();

    // r1 is settled and leaves the lane; r2 stays open.
    settle(&store, r1.clone(), Tokens::new(5)).unwrap();

    // Re-submitting the now-stale r1 must be refused, never applied to r2's bookkeeping.
    let err = settle(&store, r1, Tokens::new(5)).unwrap_err();
    assert!(matches!(err, SettleError::UnknownReservation(_)));
}
