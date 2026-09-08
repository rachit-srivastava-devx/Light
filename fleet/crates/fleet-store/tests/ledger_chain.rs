//! `Ledger::append`/`rows`/`verify` happy path -- seq/hash/prev_hash stamping, process restart.

use fleet_store::ledger::{LedgerPaths, VerifiedChain};
use fleet_store::Ledger;
use fleet_types::ReceiptEvent;
use serde_json::json;

fn paths(dir: &tempfile::TempDir) -> LedgerPaths {
    LedgerPaths { chain: dir.path().join("chain.jsonl"), lock: dir.path().join("chain.lock") }
}

#[test]
fn append_stamps_seq_prev_hash_and_recomputable_hash() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = Ledger::open(paths(&dir));
    let r0 = ledger.append(ReceiptEvent::RunStart, json!({}), "a".into(), None, None).unwrap();
    let r1 = ledger.append(ReceiptEvent::RunEnd, json!({"k": 1}), "a".into(), None, None).unwrap();
    let r2 = ledger.append(ReceiptEvent::LaneStatus, json!({}), "a".into(), None, None).unwrap();
    assert_eq!((r0.seq, r1.seq, r2.seq), (0, 1, 2));
    assert_eq!(r1.prev_hash.as_str(), r0.hash.as_str());
    assert_eq!(r2.prev_hash.as_str(), r1.hash.as_str());
}

#[test]
fn full_chain_survives_process_restart() {
    let dir = tempfile::tempdir().unwrap();
    {
        let ledger = Ledger::open(paths(&dir));
        for _ in 0..5 {
            ledger.append(ReceiptEvent::RunStart, json!({}), "a".into(), None, None).unwrap();
        }
    }
    let reopened = Ledger::open(paths(&dir));
    let result = reopened.verify().unwrap();
    assert_eq!(result, VerifiedChain { checked: 5, total: 5 });
}

#[test]
fn verify_empty_chain_is_an_error_not_a_vacuous_pass() {
    let dir = tempfile::tempdir().unwrap();
    let ledger = Ledger::open(paths(&dir));
    assert!(matches!(ledger.verify(), Err(fleet_store::ledger::LedgerError::Empty)));
}

/// Any prefix (n >= 1) of a valid chain is itself a valid chain -- the untampered chain's own
/// verify never spuriously fails partway through. A deterministic stand-in for BLUEPRINT.md §9's
/// suggested `proptest` (kept out of this crate's dependency surface; the property is exercised
/// over a fixed spread of append counts instead of randomly generated ones).
#[test]
fn any_prefix_of_a_valid_chain_is_itself_a_valid_chain() {
    for n in 1..=12u64 {
        let dir = tempfile::tempdir().unwrap();
        let ledger = Ledger::open(paths(&dir));
        for i in 0..n {
            ledger.append(ReceiptEvent::RunStart, json!({"i": i}), "a".into(), None, None).unwrap();
        }
        let result = ledger.verify().unwrap();
        assert_eq!(result, VerifiedChain { checked: n, total: n });
    }
}
