//! `run_bounded` -- the S1-hang fix. Spawns a gate's argv with stdin closed, drains its stdout/
//! stderr pipes on background threads, and polls `try_wait` against a shared deadline; on expiry
//! the child is killed and a typed timeout `ProcessOutput` (exit 124) is returned instead of
//! blocking forever. Split out of `verify_ports.rs` for the 80-line cap.
//!
//! **S1 fix**: every spawn sets `.current_dir(repo)` -- the child no longer inherits the `fleet`
//! process's own cwd. A `Script` gate is still RESOLVED against the gates-root
//! (`verify_ports.rs::resolve_gates_root`) but now EXECUTES with `repo` as cwd, same as `OnPath`.

use super::tool_path;
use super::verify_report as report;
use super::verify_runner_io::{drain, io_error, timeout_output};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use verify::ProcessOutput;

fn repo_target_dir(repo: &Path) -> Option<String> {
    let base = std::env::var_os("CARGO_TARGET_DIR")?;
    let base = Path::new(&base);
    let base = if base.is_absolute() {
        base.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(base)
    };
    let identity = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let hash = identity
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(14_695_981_039_346_656_037u64, |state, byte| {
            (state ^ u64::from(*byte)).wrapping_mul(1_099_511_628_211)
        });
    Some(
        base.join(format!("fleet-repo-{hash:016x}"))
            .to_string_lossy()
            .into_owned(),
    )
}

fn apply_gate_environment(process: &mut Command, target_dir: Option<String>) {
    let inherited = ["PATH", "HOME", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME"];
    process.env_clear();
    for key in inherited {
        if let Some(value) = std::env::var_os(key) {
            process.env(key, value);
        }
    }
    if let Some(value) = std::env::var_os("FLEET_GATES_ROOT") {
        process.env("FLEET_GATES_ROOT", value);
    }
    if let Some(value) = target_dir {
        process.env("CARGO_TARGET_DIR", value);
    }
}

pub fn run_bounded(command: &[&str], deadline: Instant, repo: &Path) -> ProcessOutput {
    let Some((bin, rest)) = command.split_first() else {
        return ProcessOutput {
            exit_code: -1,
            stdout: String::new(),
            stderr: "empty command".into(),
        };
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        report::budget_spent(bin);
        return timeout_output(command, remaining);
    }
    report::running(&command.join(" "), remaining);
    // The probe may have found this tool outside `$PATH` (rustup's `~/.cargo/bin`); spawning the
    // bare name would then fail with ENOENT right after the probe said it was available.
    let bin = tool_path::resolve_bin(bin);
    let mut process = Command::new(&bin);
    process
        .args(rest)
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_gate_environment(&mut process, repo_target_dir(repo));
    let mut child = match process.spawn() {
        Ok(c) => c,
        Err(e) => return io_error(e),
    };
    let out_rx = child.stdout.take().map(drain);
    let err_rx = child.stderr.take().map(drain);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = out_rx
                    .and_then(|r| r.recv_timeout(Duration::from_secs(2)).ok())
                    .unwrap_or_default();
                let stderr = err_rx
                    .and_then(|r| r.recv_timeout(Duration::from_secs(2)).ok())
                    .unwrap_or_default();
                return ProcessOutput {
                    exit_code: status.code().unwrap_or(-1),
                    stdout,
                    stderr,
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    // Do NOT block on child.wait() here: cargo (and similar tools) acquire
                    // the cargo build lock via flock(), which puts the process in uninterruptible
                    // sleep (D state). SIGKILL is queued but not delivered until flock() wakes,
                    // which can take many seconds while other cargo processes hold the lock.
                    // Reap the zombie on a background thread so we return immediately.
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                    report::timed_out(&bin);
                    return timeout_output(command, remaining);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return io_error(e),
        }
    }
}
