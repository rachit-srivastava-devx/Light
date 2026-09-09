//! The fd-3 protocol core: `socketpair(AF_UNIX, SOCK_SEQPACKET)` with a Darwin `SOCK_STREAM`
//! fallback, `dup2`'d onto fd 3 in the child before exec. Ported from
//! `main.rs::spawn_agent_with_args`'s socket half. Receive-side helpers live in `fd3_recv.rs`.

use nix::sys::resource::{setrlimit, Resource};
use std::os::raw::c_int;
use std::os::unix::process::CommandExt;
use std::process::Command;

#[path = "fd3_recv.rs"]
mod fd3_recv;
pub use fd3_recv::{recv, validate_submission};

#[path = "fd3_send.rs"]
mod fd3_send;
pub use fd3_send::{child_channel_open, send_done, send_refuse};

/// One megabyte of address space headroom is generous for a CLI wrapper process; a runaway
/// lane is bounded rather than left to exhaust the host. New supervision beyond what `main.rs`
/// does today (BLUEPRINT.md §7's `nix` rationale).
const LANE_RLIMIT_AS_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// A parent/child fd-3 socketpair, already `dup2`-wired into `command`'s `pre_exec`. The caller
/// must close `child_fd` in the parent after `spawn()` returns (on both success and failure) --
/// the child keeps its own copy via `dup2`, but a `fork()` also hands the parent a live copy
/// that must not leak.
pub struct Fd3Channel {
    pub parent_fd: c_int,
    pub child_fd: c_int,
}

/// `socketpair()` + arrange for the child's end to land on fd 3 before exec, plus a per-lane
/// `RLIMIT_AS`. Returns `None` if `socketpair()` fails on both attempts.
pub fn wire(command: &mut Command) -> Option<Fd3Channel> {
    let mut fds = [0 as c_int; 2];
    let mut rc =
        unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, fds.as_mut_ptr()) };
    if rc != 0 {
        rc = unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
    }
    if rc != 0 {
        return None;
    }
    let parent_fd = fds[0];
    let child_fd = fds[1];
    unsafe {
        command.pre_exec(move || {
            // Captured before any fork happens in this closure: at this point the direct parent
            // is the fleet process itself (see `parent_watch`'s doc comment).
            let fleet_pid = nix::unistd::Pid::from_raw(libc::getppid());
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            super::parent_watch::spawn_parent_death_watchdog(fleet_pid)?;
            let _ = setrlimit(Resource::RLIMIT_AS, LANE_RLIMIT_AS_BYTES, LANE_RLIMIT_AS_BYTES);
            if child_fd != 3 && libc::dup2(child_fd, 3) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if child_fd != 3 {
                libc::close(child_fd);
            }
            Ok(())
        });
    }
    Some(Fd3Channel { parent_fd, child_fd })
}
