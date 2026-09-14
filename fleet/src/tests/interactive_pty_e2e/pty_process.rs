//! Spawns the real `fleet` binary with a real pty slave as its controlling terminal --
//! `setsid` + `TIOCSCTTY`, matching what a real terminal emulator does when it spawns a shell.

use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

fn fleet() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fleet"))
}

/// One more fd onto the same slave device, so closing the parent's original `slave` after
/// spawn doesn't also close the child's copy out from under it.
fn dup_owned(fd: &OwnedFd) -> OwnedFd {
    let raw = nix::unistd::dup(fd.as_raw_fd()).expect("dup slave pty fd");
    unsafe { OwnedFd::from_raw_fd(raw) }
}

/// Spawns `fleet` with no args, its stdin/stdout/stderr attached to a real pty slave as its
/// controlling terminal. Returns the child and a `File` for the pty master (read + write).
pub(super) fn spawn_fleet_on_pty(state_dir: &std::path::Path) -> (std::process::Child, File) {
    // A real, non-zero size: real terminals always report one, and a 0x0 window is an
    // unrealistic edge case this test has no interest in covering.
    let winsize = nix::pty::Winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let pty = nix::pty::openpty(Some(&winsize), None).expect("openpty");
    let slave_raw = pty.slave.as_raw_fd();

    let mut cmd = fleet();
    cmd.env("FLEET_STATE_DIR", state_dir)
        .env("FLEET_LOAD_FACTOR", "10000")
        .env("NO_COLOR", "1")
        .stdin(Stdio::from(dup_owned(&pty.slave)))
        .stdout(Stdio::from(dup_owned(&pty.slave)))
        .stderr(Stdio::from(dup_owned(&pty.slave)));

    // SAFETY: only async-signal-safe calls between fork and exec (setsid + ioctl), as required
    // by `pre_exec`'s own contract.
    unsafe {
        cmd.pre_exec(move || {
            nix::unistd::setsid().map_err(std::io::Error::from)?;
            if libc::ioctl(slave_raw, libc::TIOCSCTTY as _, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let child = cmd.spawn().expect("fleet must spawn under a pty");
    // Parent's own slave-side fds are no longer needed once the child has its dup'd copies;
    // holding them open would keep the pty session alive (and `master` reading forever) even
    // after the child exits.
    drop(pty.slave);
    (child, File::from(pty.master))
}
