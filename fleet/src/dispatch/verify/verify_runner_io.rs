//! Small `ProcessOutput` builders + the pipe-drain helper for `verify_runner_bounded.rs`, split
//! out purely to keep that file under the 80-line cap after the S1 `.current_dir(repo)` fix grew
//! its doc comment and signature.

use std::io::Read;
use std::sync::mpsc;
use std::time::Duration;
use verify::ProcessOutput;

pub fn io_error(e: std::io::Error) -> ProcessOutput {
    ProcessOutput {
        exit_code: -1,
        stdout: String::new(),
        stderr: e.to_string(),
    }
}

pub fn timeout_output(command: &[&str], budget: Duration) -> ProcessOutput {
    let joined = command.join(" ");
    ProcessOutput {
        exit_code: 124,
        stdout: String::new(),
        stderr: format!("fleet: verify: `{joined}` exceeded the {budget:?} verify budget, killed"),
    }
}

/// Read a pipe to completion on a background thread, delivering the collected text over a
/// channel so the poll loop in `run_bounded` never blocks on a `read` directly.
pub fn drain(pipe: impl Read + Send + 'static) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        const MAX_BYTES: usize = 8 * 1024 * 1024;
        let mut bytes = Vec::with_capacity(MAX_BYTES);
        let _ = pipe.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes);
        let truncated = bytes.len() > MAX_BYTES;
        bytes.truncate(MAX_BYTES);
        let mut buf = String::from_utf8_lossy(&bytes).into_owned();
        if truncated {
            buf.push_str("\n[fleet: gate output truncated at 8 MiB]");
        }
        let _ = tx.send(buf);
    });
    rx
}
