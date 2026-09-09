//! Parent-death watchdog. macOS has no `PR_SET_PDEATHSIG` (Linux-only), and the worker child
//! already runs in its own session (`setsid()` in `fd3::wire`) so a `kill -9` on the fleet
//! parent process is never propagated to it -- nothing reaps an orphaned worker. This forks ONE
//! sibling inside the same `pre_exec` that calls `setsid()`, whose only job is to poll whether
//! the fleet process that spawned it is still alive and, the moment it is not, `SIGKILL` the
//! whole process group (itself and the real worker both) instead of leaving it running forever.
//!
//! Must be called AFTER `setsid()` (so the watchdog inherits the new session's pgid and a
//! group-kill takes it down too) and BEFORE the caller's `pre_exec` returns, with `fleet_pid`
//! captured via `libc::getppid()` at the top of that same closure -- i.e. before this or any
//! other fork happens, while the direct parent is still the fleet process itself.

use nix::sys::signal::{kill, Signal};
use nix::unistd::{fork, getpgrp, getppid, ForkResult, Pid};
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Forks once. The parent of this fork returns `Ok(())` and falls through to `dup2`/`exec` the
/// real worker, unaffected. The child never returns -- it becomes the watchdog and loops until
/// `fleet_pid` is gone, then kills the group and `_exit`s. Only raw syscalls run in the child
/// (no allocation, no other locks) since it exists solely by `fork()`, never `exec()`.
pub fn spawn_parent_death_watchdog(fleet_pid: Pid) -> std::io::Result<()> {
    match unsafe { fork() } {
        Ok(ForkResult::Child) => {
            close_inherited_fds();
            watchdog_loop(fleet_pid);
        }
        Ok(ForkResult::Parent { .. }) => Ok(()),
        Err(errno) => Err(errno.into()),
    }
}

/// `fork()` duplicates every open file descriptor, including ones this code cannot see: the
/// fd-3 channel the caller is about to `dup2`, and -- critically -- Rust std's own internal
/// close-on-exec pipe that `Command::spawn` reads from in the FLEET PARENT to learn whether
/// `exec()` succeeded. That pipe closes when the process holding it either closes it or execs;
/// this watchdog does neither on its own, so without this, a copy of that write end survives
/// forever in the watchdog and the fleet parent's `spawn()` call hangs waiting for an EOF that
/// never comes -- even though the real worker exec'd fine. Closing every fd above stderr, first
/// thing, is the standard fix (the same one daemonizing code has always needed): a `close()` on
/// an fd this process never had is just `EBADF`, silently ignored, never a partial failure.
fn close_inherited_fds() {
    for fd in 3..1024 {
        unsafe {
            libc::close(fd);
        }
    }
}

/// `kill(pid, 0)` is the portable liveness probe: no signal delivered, only `ESRCH` (gone) vs.
/// success (still alive, or alive but unsignallable -- treated as alive, the conservative call).
/// Known limitation: pid reuse after the fleet process exits could in principle fool this: a
/// long-lived, slow-polling watchdog outlasting a pid recycle onto an unrelated process is an
/// accepted, rare race, not solved here.
///
/// Also self-retires the ordinary way, so this never outlives every lane it ever guarded until
/// fleet itself exits: `getppid()` at this point is the worker sibling's pid (this watchdog's
/// actual parent, unaffected by that sibling's later `exec()`). The same portable
/// "poll getppid(), notice it change" trick the module doc describes for detecting the FLEET
/// parent's death also detects the ordinary case -- the worker finished and was reaped by
/// `join` -- reparenting this watchdog to init. That is success, not an emergency: exit quietly,
/// no signal sent.
fn watchdog_loop(fleet_pid: Pid) -> ! {
    let worker_pid = getppid();
    loop {
        std::thread::sleep(POLL_INTERVAL);
        if getppid() != worker_pid {
            unsafe { libc::_exit(0) };
        }
        if kill(fleet_pid, None).is_err() {
            let _ = kill(Pid::from_raw(-getpgrp().as_raw()), Signal::SIGKILL);
            unsafe { libc::_exit(0) };
        }
    }
}
