//! Process-group supervision: every spawned child gets its own session (`setsid`) so a timeout
//! kill can target the whole group (`kill(-pid, ...)`), never just the direct child -- ported
//! from `main.rs:2953-2963`/`3157-3192`.

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::process::Child;
use std::time::{Duration, Instant};

// The `setsid()` `pre_exec` hook itself lives in `fd3::wire` (it must run in the SAME pre_exec
// chain as the fd-3 `dup2`, and Rust's `pre_exec` closures run in unspecified relative order
// otherwise) -- this module owns only the wait/kill half of process-group supervision.

/// Outcome of waiting for a child within `deadline`.
pub enum WaitOutcome {
    Exited,
    TimedOut,
}

/// Poll-with-deadline; on timeout escalate SIGTERM then (5s grace) SIGKILL to the whole process
/// group (`-pid`), never the single child pid. Ported from `main.rs::wait_with_deadline`.
pub fn wait_with_deadline(child: &mut Child, deadline: Duration) -> WaitOutcome {
    let until = Instant::now() + deadline;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return WaitOutcome::Exited,
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => break,
            Err(_) => return WaitOutcome::TimedOut,
        }
    }
    terminate_group(child.id() as i32, Signal::SIGTERM);
    let kill_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return WaitOutcome::TimedOut,
            Ok(None) if Instant::now() < kill_deadline => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Ok(None) => {
                terminate_group(child.id() as i32, Signal::SIGKILL);
                let _ = child.wait();
                return WaitOutcome::TimedOut;
            }
            Err(_) => return WaitOutcome::TimedOut,
        }
    }
}

/// Signal the whole process group rooted at `pid`, not just the direct child.
fn terminate_group(pid: i32, signal: Signal) {
    let _ = kill(Pid::from_raw(-pid), signal);
}
