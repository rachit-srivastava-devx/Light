//! Real-binary verification receipts: refusal and unborn-Git fallback must be durable.
#[path = "../support/mod.rs"]
mod support;

use std::path::Path;
use std::process::Output;
use store::Ledger;
use store::ledger::LedgerPaths;
use types::ReceiptEvent;

fn ledger(dir: &Path) -> Ledger {
    Ledger::open(LedgerPaths {
        chain: dir.join("ledger.chain"),
        lock: dir.join("ledger.lock"),
    })
}

fn run(state: &Path, args: &[&str]) -> Output {
    support::cmd()
        .env("FLEET_STATE_DIR", state)
        .env("FLEET_VERIFY_BUDGET_SECS", "30")
        .env("FLEET_GITLEAKS_BUDGET_SECS", "30")
        .args(args)
        .output()
        .expect("fleet binary runs")
}

#[test]
fn invalid_target_writes_a_typed_refusal_receipt() {
    let state = tempfile::tempdir().expect("state");
    let out = run(state.path(), &["gate", "--repo", "/not/a/real/fleet/repo"]);
    assert_eq!(out.status.code(), Some(3));
    let rows = ledger(state.path()).rows(false).expect("receipt rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].event, ReceiptEvent::Refusal);
    assert_eq!(rows[0].exit_code, Some(types::ExitCode::Env));
    assert_eq!(rows[0].body["outcome"], "refused");
}

#[path = "unborn_tests.rs"]
mod unborn;
