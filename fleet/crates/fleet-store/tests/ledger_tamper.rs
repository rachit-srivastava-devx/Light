//! Each tamper class is caught with the specific `LedgerError` variant it corresponds to.

use fleet_store::ledger::{LedgerError, LedgerPaths};
use fleet_store::Ledger;
use fleet_types::ReceiptEvent;
use serde_json::json;

mod support;
use support::{lines_of, seeded, write_lines};

#[test]
fn append_refuses_to_extend_an_already_broken_chain() {
    let dir = tempfile::tempdir().unwrap();
    let p = seeded(&dir, 1);
    let mut rows = lines_of(&p.chain);
    rows[0]["hash"] = json!(format!("blake3:{}", "0".repeat(64)));
    write_lines(&p.chain, &rows);

    let ledger = Ledger::open(LedgerPaths { chain: p.chain.clone(), lock: p.lock.clone() });
    let err = ledger.append(ReceiptEvent::RunEnd, json!({}), "a".into(), None, None).unwrap_err();
    assert!(matches!(err, LedgerError::Tampered { seq: 0 }));
}

#[test]
fn reordered_rows_are_caught_as_seq_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let p = seeded(&dir, 3);
    let mut rows = lines_of(&p.chain);
    rows.swap(0, 1);
    write_lines(&p.chain, &rows);

    let ledger = Ledger::open(p);
    assert!(matches!(ledger.verify().unwrap_err(), LedgerError::SeqMismatch { seq: 0, .. }));
}

/// A duplicated `prev_hash` claim is rejected. Note: given the check order (seq, then
/// prev-hash-linkage, then the `seen_prev` fork check, then content-hash), a genuine `Fork`
/// return requires two *different* rows to independently produce the identical blake3 hash for
/// their predecessor -- which the linkage check already rules out for any single-file corruption
/// that doesn't also break linkage first. This asserts the corruption is caught by whichever
/// variant fires first; `BrokenLink` is the reachable one here, `Fork` is defense-in-depth for a
/// hash collision this test cannot manufacture.
#[test]
fn a_duplicated_prev_hash_claim_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let p = seeded(&dir, 3);
    let mut rows = lines_of(&p.chain);
    let genesis_prev = rows[0]["prev_hash"].clone();
    rows[1]["prev_hash"] = genesis_prev;
    write_lines(&p.chain, &rows);

    let ledger = Ledger::open(p);
    assert!(matches!(ledger.verify().unwrap_err(), LedgerError::BrokenLink { .. } | LedgerError::Fork { .. }));
}

#[test]
fn a_hand_edited_body_is_caught_as_tampered() {
    let dir = tempfile::tempdir().unwrap();
    let p = seeded(&dir, 2);
    let mut rows = lines_of(&p.chain);
    rows[1]["body"] = json!({"i": 999});
    write_lines(&p.chain, &rows);

    let ledger = Ledger::open(p);
    assert!(matches!(ledger.verify().unwrap_err(), LedgerError::Tampered { seq: 1 }));
}
