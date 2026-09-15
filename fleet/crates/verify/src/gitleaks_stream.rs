use std::io::Read;
use std::process::{ChildStderr, ChildStdout};
use std::thread::{self, JoinHandle};

pub(super) fn drain_stdout(stream: ChildStdout) -> JoinHandle<Vec<u8>> {
    drain(stream)
}

pub(super) fn drain_stderr(stream: ChildStderr) -> JoinHandle<Vec<u8>> {
    drain(stream)
}

fn drain(mut stream: impl Read + Send + 'static) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        const MAX: usize = 8 * 1024;
        let mut kept = Vec::with_capacity(MAX);
        let mut buffer = [0u8; 4096];
        while let Ok(read) = stream.read(&mut buffer) {
            if read == 0 {
                break;
            }
            let take = (MAX - kept.len()).min(read);
            kept.extend_from_slice(&buffer[..take]);
        }
        kept
    })
}
