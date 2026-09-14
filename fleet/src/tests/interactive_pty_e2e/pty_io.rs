//! Reads the pty master on a background thread, standing in for the terminal emulator a real
//! user would have: reedline calls `crossterm::cursor::position()` while painting (its own doc
//! comment: "is blocking and can time out"), which sends a cursor-position query (`ESC[6n`) and
//! waits for a reply. Answering it, as any real terminal does, lets reedline's actual raw-mode
//! read loop run instead of timing out and falling back to a line-buffered reader.

use std::fs::File;
use std::io::Read;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const CURSOR_POSITION_QUERY: &[u8] = b"\x1b[6n";

pub(super) fn spawn_reader_and_responder(mut master: File) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut tail: Vec<u8> = Vec::new();
        loop {
            match master.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = &buf[..n];
                    tail.extend_from_slice(chunk);
                    while let Some(pos) = find(&tail, CURSOR_POSITION_QUERY) {
                        let _ = std::io::Write::write_all(&mut master, b"\x1b[24;80R");
                        tail.drain(..pos + CURSOR_POSITION_QUERY.len());
                    }
                    // Bound the rolling window: only needed to catch a query split across two
                    // reads, never to accumulate the whole session's output.
                    let keep_from = tail.len().saturating_sub(CURSOR_POSITION_QUERY.len() - 1);
                    tail.drain(..keep_from);
                    if tx.send(chunk.to_vec()).is_err() {
                        break;
                    }
                }
                // A pty master commonly reports EIO instead of a clean EOF once every slave fd
                // is closed; both mean "the stream ended", not a test failure by themselves.
                Err(_) => break,
            }
        }
    });
    rx
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Accumulates output from `rx` until `needle` appears or `timeout` elapses. Returns everything
/// captured so far either way, so a failing assertion can show real output, not just "timed out".
pub(super) fn wait_for(
    rx: &mpsc::Receiver<Vec<u8>>,
    needle: &str,
    timeout: Duration,
) -> (bool, String) {
    let deadline = Instant::now() + timeout;
    let mut output = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match rx.recv_timeout(remaining.min(Duration::from_millis(200))) {
            Ok(chunk) => {
                output.extend_from_slice(&chunk);
                let text = String::from_utf8_lossy(&output);
                if text.contains(needle) {
                    return (true, text.into_owned());
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    (false, String::from_utf8_lossy(&output).into_owned())
}
