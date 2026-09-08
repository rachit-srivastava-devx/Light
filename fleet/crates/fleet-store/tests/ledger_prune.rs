//! `Ledger::prune()`: always refused (append-only, hash-chained), and the chain stays verifiable
//! afterwards because nothing was ever touched.

use fleet_store::ledger::LedgerError;
use fleet_store::{Ledger, RetentionError, RetentionPolicy};
use fleet_types::ReceiptEvent;
use serde_json::json;

mod support;
use support::{paths, seeded};

#[test]
fn ledger_prune_is_always_refused_and_the_chain_stays_verifiable() {
    let dir = tempfile::tempdir().unwrap();
    let p = seeded(&dir, 5);
    let ledger = Ledger::open(fleet_store::ledger::LedgerPaths { chain: p.chain.clone(), lock: p.lock.clone() });

    let usage = ledger.usage().unwrap();
    assert_eq!(usage.row_count, 5);

    let err = ledger.prune(&RetentionPolicy { max_rows: Some(1), ..Default::default() }).unwrap_err();
    assert!(matches!(err, LedgerError::LedgerRefused));

    let verified = ledger.verify().unwrap();
    assert_eq!(verified.checked, 5);
    assert_eq!(verified.total, 5);
}

#[test]
fn retention_error_wraps_ledger_refusal() {
    let dir = tempfile::tempdir().unwrap();
    let p = paths(&dir);
    let ledger = Ledger::open(fleet_store::ledger::LedgerPaths { chain: p.chain.clone(), lock: p.lock.clone() });
    ledger.append(ReceiptEvent::RunStart, json!({}), "a".into(), None, None).unwrap();

    let err: RetentionError = ledger.prune(&RetentionPolicy::default()).unwrap_err().into();
    assert!(matches!(err, RetentionError::Ledger(LedgerError::LedgerRefused)));
}
