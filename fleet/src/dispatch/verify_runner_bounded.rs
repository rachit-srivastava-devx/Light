//! `run_bounded` -- the S1-hang fix. Spawns a gate's argv with stdin closed, drains its stdout/
//! stderr pipes on background threads, and polls `try_wait` against a shared deadline; on expiry
//! the child is killed and a typed timeout `ProcessOutput` (exit 124) is returned instead of
//! blocking forever. Split out of `verify_ports.rs` for the 80-line cap.
//!
//! **S1 fix**: every spawn sets `.current_dir(repo)` -- the child no longer inherits the `fleet`
//! process's own cwd. A `Script` gate is still RESOLVED against the gates-root
//! (`verify_ports.rs::resolve_gates_root`) but now EXECUTES with `repo` as cwd, same as `OnPath`.

use super::verify_report as report;
use super::verify_runner_io::{drain, io_error, timeout_output};
use fleet_verify::ProcessOutput;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn run_bounded(command: &[&str], deadline: Instant, repo: &Path) -> ProcessOutput {
    let Some((bin, rest)) = command.split_first() else {
        return ProcessOutput { exit_code: -1, stdout: String::new(), stderr: "empty command".into() };
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        report::budget_spent(bin);
        return timeout_output(command, remaining);
    }
    report::running(&command.join(" "), remaining);
    let mut child = match Command::new(bin)
        .args(rest)
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return io_error(e),
    };
    let out_rx = child.stdout.take().map(drain);
    let err_rx = child.stderr.take().map(drain);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = out_rx.and_then(|r| r.recv_timeout(Duration::from_secs(2)).ok()).unwrap_or_default();
                let stderr = err_rx.and_then(|r| r.recv_timeout(Duration::from_secs(2)).ok()).unwrap_or_default();
                return ProcessOutput { exit_code: status.code().unwrap_or(-1), stdout, stderr };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    report::timed_out(bin);
                    return timeout_output(command, remaining);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return io_error(e),
        }
    }
}
