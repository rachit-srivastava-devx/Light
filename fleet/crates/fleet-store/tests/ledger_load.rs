//! Ledger integrity at volume. Ignored by default (writes/verifies 120k rows); run explicitly:
//!   cargo test -p fleet-store --test ledger_load -- --ignored --nocapture
//! See `ledger_append_cost.rs` for `Ledger::append`'s cost-growth measurement.
use std::time::Instant;

use fleet_store::ledger::LedgerError;
use fleet_store::Ledger;
use serde_json::json;

mod support;
use support::bulk::build_chain;
use support::{paths, write_lines};

const N: u64 = 120_000;

#[test]
#[ignore]
fn verify_120k_records_then_detects_a_tampered_middle_record() {
    let dir = tempfile::tempdir().unwrap();
    let p = paths(&dir);
    let rows = build_chain(N);
    write_lines(&p.chain, &rows);

    let ledger = Ledger::open(fleet_store::ledger::LedgerPaths { chain: p.chain.clone(), lock: p.lock.clone() });

    let t0 = Instant::now();
    let verified = ledger.verify().unwrap();
    let clean_elapsed = t0.elapsed();
    assert_eq!(verified.checked, N);
    assert_eq!(verified.total, N);
    println!("verify({N} clean rows) took {clean_elapsed:?}");

    // Tamper with the row in the middle.
    let tamper_seq = N / 2;
    let mut tampered = rows.clone();
    tampered[tamper_seq as usize]["body"] = json!({"i": tamper_seq, "tampered": true});
    write_lines(&p.chain, &tampered);

    let t1 = Instant::now();
    let err = ledger.verify().unwrap_err();
    let tamper_elapsed = t1.elapsed();
    println!("verify({N} rows, 1 tampered) took {tamper_elapsed:?}");
    assert!(matches!(err, LedgerError::Tampered { seq } if seq == tamper_seq));
}
