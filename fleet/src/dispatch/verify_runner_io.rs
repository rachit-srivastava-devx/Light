//! Small `ProcessOutput` builders + the pipe-drain helper for `verify_runner_bounded.rs`, split
//! out purely to keep that file under the 80-line cap after the S1 `.current_dir(repo)` fix grew
//! its doc comment and signature.

use fleet_verify::ProcessOutput;
use std::io::Read;
use std::sync::mpsc;
use std::time::Duration;

pub fn io_error(e: std::io::Error) -> ProcessOutput {
    ProcessOutput { exit_code: -1, stdout: String::new(), stderr: e.to_string() }
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
pub fn drain(mut pipe: impl Read + Send + 'static) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = pipe.read_to_string(&mut buf);
        let _ = tx.send(buf);
    });
    rx
}
