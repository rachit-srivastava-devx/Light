//! `fleet ledger|rollback`: parse -> `store::Ledger`/`integrate::remove` -> print.

use cli::args_ops::{LedgerArgs, RollbackArgs};
use crate::dispatch::error::DispatchError;
use print::human;
use integrate::Worktree;
use std::path::{Path, PathBuf};
use store::ledger::LedgerPaths;
use store::Ledger;

#[derive(serde::Serialize)]
struct LedgerReport {
    checked: Option<u64>,
    total: Option<u64>,
    rows: Option<usize>,
}

pub fn ledger(state_dir: &Path, args: LedgerArgs) -> Result<(), DispatchError> {
    let paths = LedgerPaths {
        chain: state_dir.join("ledger.chain"),
        lock: state_dir.join("ledger.lock"),
    };
    let ledger = Ledger::open(paths);
    if args.verify {
        let chain = ledger
            .verify()
            .map_err(|e| DispatchError::Refusal(e.to_string()))?;
        if args.json {
            let report = LedgerReport {
                checked: Some(chain.checked),
                total: Some(chain.total),
                rows: None,
            };
            print::json::print_pretty(&report);
        } else {
            human::line("verified", format!("{}/{}", chain.checked, chain.total));
        }
    } else {
        let rows = ledger
            .rows(true)
            .map_err(|e| DispatchError::Refusal(e.to_string()))?;
        if args.json {
            let report = LedgerReport {
                checked: None,
                total: None,
                rows: Some(rows.len()),
            };
            print::json::print_pretty(&report);
        } else {
            human::line("rows", rows.len());
        }
    }
    Ok(())
}

pub fn rollback(args: RollbackArgs) -> Result<(), DispatchError> {
    let worktree = Worktree {
        path: PathBuf::from(&args.worktree),
        branch: format!("fleet/{}", args.worktree),
        name: args.worktree.clone(),
    };
    integrate::remove(Path::new(&args.repo), &worktree)?;
    human::ok("worktree removed");
    Ok(())
}
