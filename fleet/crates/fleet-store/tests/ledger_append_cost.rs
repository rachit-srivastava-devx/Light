//! `Ledger::append`'s cost-growth measurement. Split out of `ledger_load.rs` to stay under this
//! repo's 80-line-per-file limit. Ignored by default; run explicitly:
//!   cargo test -p fleet-store --test ledger_append_cost -- --ignored --nocapture
use std::time::Instant;

use fleet_store::Ledger;
use fleet_types::ReceiptEvent;
use serde_json::json;

mod support;
use support::paths;

/// `Ledger::append` reads only the on-disk tip (not the whole chain) to learn where to extend,
/// so N appends cost O(N) total. This measures that growth directly: if the cost is O(N)
/// amortized, doubling N should roughly double the time (~2x); a regression back to the old
/// O(N^2) re-verify-everything behavior would roughly quadruple it (~4x) per doubling instead.
/// Asserts the ratio stays well under the quadratic signature, so a regression fails the suite
/// instead of merely being slow. Formerly `append_cost_grows_quadratically_with_chain_length`
/// (renamed: that name described the old, now-fixed, O(N^2) behavior).
#[test]
#[ignore]
fn append_cost_grows_linearly_with_chain_length() {
    let dir = tempfile::tempdir().unwrap();
    let p = paths(&dir);
    let ledger = Ledger::open(fleet_store::ledger::LedgerPaths { chain: p.chain.clone(), lock: p.lock.clone() });

    let checkpoints = [1_000u64, 2_000, 4_000, 8_000];
    let mut last_elapsed = None;
    let mut appended = 0u64;
    for target in checkpoints {
        let t0 = Instant::now();
        while appended < target {
            ledger.append(ReceiptEvent::RunStart, json!({"i": appended}), "a".into(), None, None).unwrap();
            appended += 1;
        }
        let elapsed = t0.elapsed();
        println!("appending rows up to {target} took {elapsed:?}");
        if let Some(prev) = last_elapsed {
            let ratio: f64 = elapsed.as_secs_f64() / prev;
            println!("  ratio vs previous doubling: {ratio:.2}x (linear ~2x, quadratic ~4x)");
            assert!(
                ratio < 3.0,
                "append cost ratio {ratio:.2}x per doubling looks quadratic, not linear -- \
                 did append() start re-reading/re-verifying the whole chain again?"
            );
        }
        last_elapsed = Some(elapsed.as_secs_f64());
    }
}
