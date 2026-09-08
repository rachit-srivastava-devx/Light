//! Test-only fixture standing in for the real `src/` `__agent` child dispatch, driven only via
//! `FLEET_WORKER_TEST_CHILD_EXE` (see `src/spawn/util.rs`). Never shipped/invoked in production.
//! Argv mirrors the real shape: `__agent <kind> <worktree_path> <task> [model]`. The task's first
//! whitespace-delimited word selects the scenario this run acts out.

mod fixture_scenarios;

use std::env;

fn main() {
    // argv: [0]=exe [1]="__agent" [2]=kind [3]=worktree_path [4]=task [5]=model?
    let args: Vec<String> = env::args().collect();
    let task = args.get(4).cloned().unwrap_or_default();
    let scenario = task.split_whitespace().next().unwrap_or("done");
    match scenario {
        "done" => fixture_scenarios::send_done(),
        "refuse" => fixture_scenarios::send_refuse(),
        "malformed" => fixture_scenarios::send_malformed(),
        "silent" => fixture_scenarios::exit_silently(),
        "timeout" => fixture_scenarios::hang_with_grandchild(&args),
        "envdump" => fixture_scenarios::send_env_dump(),
        other => {
            eprintln!("fw-fixture-agent: unknown scenario {other:?}");
            std::process::exit(1);
        }
    }
}
