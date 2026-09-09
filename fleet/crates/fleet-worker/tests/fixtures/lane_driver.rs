//! Manual-only harness for the `kill -9` orphan/reap demonstration (never run by `cargo test`;
//! not wired into any production path). It plays the role of "the fleet process" itself so a
//! shell session can `kill -9` IT from outside and observe whether the worker it spawned
//! survives unsupervised (the bug) or is reaped by the parent-death watchdog (the fix).
//!
//! `spawn <repo>` -- spawn one "timeout" lane (the existing fixture scenario that forks a
//! sleeping grandchild), print `<worktree_path> <worker_pid>`, then block forever. Never calls
//! `join`, matching a real fleet process that got `kill -9`'d mid-run.
//! `reap <repo>`  -- one-shot: print, then actually remove, every lane `reap_dead_lanes` proves
//! dead under `<repo>/.worktrees`.

use fleet_types::TaskId;
use fleet_worker::{find_dead_lanes, reap_dead_lanes, spawn, CliAdapter, Role, SpawnRequest};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match (args.get(1).map(String::as_str), args.get(2)) {
        (Some("spawn"), Some(repo)) => do_spawn(repo.into()),
        (Some("reap"), Some(repo)) => do_reap(repo.into()),
        _ => {
            eprintln!("usage: fw-lane-driver <spawn|reap> <repo>");
            std::process::exit(2);
        }
    }
}

fn do_spawn(repo: PathBuf) {
    eprintln!("driver: start"); std::io::stderr().flush().ok();
    let fixture = std::env::var("FW_FIXTURE_AGENT_EXE").expect("FW_FIXTURE_AGENT_EXE must be set");
    std::env::set_var("FLEET_WORKER_TEST_CHILD_EXE", fixture);
    let request = SpawnRequest {
        repo,
        role: Role::Builder,
        task_id: TaskId::parse("t1").unwrap(),
        adapter: CliAdapter::Freelane,
        requested_model: None,
        task: "timeout".to_string(),
        deadline: Duration::from_secs(600),
    };
    eprintln!("driver: calling spawn()"); std::io::stderr().flush().ok();
    let handle = spawn(request).expect("spawn");
    eprintln!("driver: spawn() returned"); std::io::stderr().flush().ok();
    let pid_file = handle.worktree_path.join(".fleet-lane.pid");
    let pid = std::fs::read_to_string(&pid_file).unwrap_or_default();
    println!("{} {}", handle.worktree_path.display(), pid.trim());
    std::io::stdout().flush().ok();
    eprintln!("driver: printed, looping"); std::io::stderr().flush().ok();
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

fn do_reap(repo: PathBuf) {
    for path in find_dead_lanes(&repo) {
        println!("dead: {}", path.display());
    }
    for (path, result) in reap_dead_lanes(&repo) {
        println!("reaped {}: {:?}", path.display(), result.is_ok());
    }
}
