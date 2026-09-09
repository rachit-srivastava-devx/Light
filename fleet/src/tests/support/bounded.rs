//! Shared "spawn the real binary, bounded by wall-clock time" helper -- split out so tests that
//! drive the real compiled `fleet` binary (never a fake seam, see `hangs_terminate_real_binary.rs`)
//! don't each need their own copy just to stay under the 80-line-per-file cap.

use super::cmd;
use std::io::Read;
use std::process::Stdio;
use std::time::{Duration, Instant};

/// Run `bin() args...` with `stdin` closed, bounded to `budget` wall-clock time. `None` means the
/// deadline was hit and the child was killed -- a regression back to a hang, not a pass.
pub fn run_bounded(args: &[&str], envs: &[(&str, &str)], budget: Duration) -> Option<(i32, String, String)> {
    let mut c = cmd();
    c.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in envs {
        c.env(k, v);
    }
    let mut child = c.spawn().expect("binary spawns");
    let deadline = Instant::now() + budget;
    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            let mut out = String::new();
            let mut err = String::new();
            if let Some(mut o) = child.stdout.take() {
                let _ = o.read_to_string(&mut out);
            }
            if let Some(mut e) = child.stderr.take() {
                let _ = e.read_to_string(&mut err);
            }
            return Some((status.code().unwrap_or(-1), out, err));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
