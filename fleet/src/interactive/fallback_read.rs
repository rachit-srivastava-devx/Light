//! Byte-oriented, EOF-safe fallback reader used when reedline's raw-mode read fails -- e.g. a
//! terminal that never answers a cursor-position query, so `crossterm::cursor::position()`
//! times out (reedline's own doc comment on that call notes it "is blocking and can time out").
//! Plain `read_line` cannot be trusted here: raw mode may still be active on the tty, so Ctrl-D
//! arrives as a literal `0x04` byte rather than a kernel-level EOF, and a line-buffered read
//! would block forever waiting for a newline that never comes.

use super::line_reader::ReadOutcome;
use std::io::{self, Read, Write};

/// Longest line this reader will accumulate before giving up. This path is a rare last resort,
/// not the normal input loop, so a generous but finite bound is enough to stop a runaway or
/// garbled stream from growing without limit.
const MAX_LINE_BYTES: usize = 64 * 1024;

pub(super) fn fallback_read() -> ReadOutcome {
    print!("> ");
    let _ = io::stdout().flush();
    read_fallback_line(&mut io::stdin())
}

fn read_fallback_line<R: Read>(input: &mut R) -> ReadOutcome {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match input.read(&mut byte) {
            // With nothing typed yet, EOF means "quit" (canonical-mode Ctrl-D, or a stream
            // that was never going to send more). With something already typed, treat the
            // stream ending like a line terminator instead of discarding it.
            Ok(0) if line.is_empty() => return ReadOutcome::Exit,
            Ok(0) => break,
            Ok(_) => match byte[0] {
                0x04 if line.is_empty() => return ReadOutcome::Exit,
                b'\n' => break,
                _ if line.len() >= MAX_LINE_BYTES => return ReadOutcome::Exit,
                b => line.push(b),
            },
            Err(_) => return ReadOutcome::Exit,
        }
    }
    match String::from_utf8(line) {
        Ok(s) => ReadOutcome::Submit(s.trim_end_matches('\r').to_string()),
        Err(_) => ReadOutcome::Exit,
    }
}

#[cfg(test)]
#[path = "fallback_read_tests.rs"]
mod tests;
