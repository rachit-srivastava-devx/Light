//! `run_bounded` -- the S1-hang fix. Spawns a gate's argv with stdin closed, drains its stdout/
//! stderr pipes on background threads (so a chatty child cannot deadlock by filling a pipe buffer
//! nobody is reading), and polls `try_wait` against a shared deadline. On expiry the child is
//! killed and a typed timeout `ProcessOutput` (exit 124, naming the command and budget) is
//! returned instead of blocking forever. Split out of `verify_ports.rs` for the 80-line cap.

use super::verify_report as report;
use fleet_verify::ProcessOutput;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn io_error(e: std::io::Error) -> ProcessOutput {
    ProcessOutput { exit_code: -1, stdout: String::new(), stderr: e.to_string() }
}

fn timeout_output(command: &[&str], budget: Duration) -> ProcessOutput {
    let joined = command.join(" ");
    ProcessOutput {
        exit_code: 124,
        stdout: String::new(),
        stderr: format!("fleet: verify: `{joined}` exceeded the {budget:?} verify budget, killed"),
    }
}

/// Read a pipe to completion on a background thread, delivering the collected text over a
/// channel so the poll loop below never blocks on a `read` directly.
fn drain(mut pipe: impl Read + Send + 'static) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = pipe.read_to_string(&mut buf);
        let _ = tx.send(buf);
    });
    rx
}

pub fn run_bounded(command: &[&str], deadline: Instant) -> ProcessOutput {
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
