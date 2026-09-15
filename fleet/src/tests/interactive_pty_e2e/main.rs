//! Drives the real `fleet` binary over an actual pseudo-terminal to prove the interactive
//! REPL genuinely opens and renders -- not a proxy for it.
//!
//! `src/tests/interactive_startup/main.rs` proves the *other* branch: given non-terminal
//! stdin/stdout, `fleet` prints help and exits. `Command::output()` always gives the child
//! pipes, never a tty, so nothing there can prove the terminal branch actually works -- exactly
//! the class of bug `dev.sh` had (it never even ran the binary on the no-args path). See
//! `pty_process.rs` and `pty_io.rs` for how the pty and its terminal-emulator stand-in work.

#[path = "pty_io.rs"]
mod pty_io;
#[path = "pty_process.rs"]
mod pty_process;

use pty_io::{spawn_reader_and_responder, wait_for};
use pty_process::spawn_fleet_on_pty;
use std::io::Write;
use std::time::{Duration, Instant};

#[test]
fn interactive_repl_renders_and_exits_cleanly_over_a_real_tty() {
    let state_dir = tempfile::tempdir().expect("tempdir");
    let (mut child, master) = spawn_fleet_on_pty(state_dir.path());
    let rx = spawn_reader_and_responder(master.try_clone().expect("clone pty master"));
    let mut writer = master;

    // The banner and status bar are plain `println!` output emitted before reedline ever takes
    // the terminal into raw mode (src/interactive/repl.rs), so this proves the process actually
    // started rendering.
    let (saw_banner, captured) = wait_for(&rx, "System capacity healthy", Duration::from_secs(15));
    assert!(
        saw_banner,
        "expected the banner/status bar to render over a real tty; captured so far:\n{captured}"
    );
    assert!(
        captured.contains("Fleet") && captured.contains("Local Orchestration"),
        "expected the Claude-Code-style banner (title + mode line); captured:\n{captured}"
    );

    // reedline draws its own prompt only once raw mode is active and its cursor-position query
    // (answered by pty_io's responder) has returned. Waiting for the real "› " prompt -- rather
    // than sending Ctrl-D the instant the banner appears -- proves the real reedline path
    // responded, not the line-buffered fallback, and avoids racing the raw-mode transition.
    let (saw_prompt, captured) = wait_for(&rx, "\u{203a} ", Duration::from_secs(10));
    assert!(
        saw_prompt,
        "expected reedline's prompt once raw mode is active; captured so far:\n{captured}"
    );

    // Ctrl-D (EOT) is how a real terminal signals end-of-input; line_reader.rs maps
    // `Signal::CtrlD` to `ReadOutcome::Exit`, which breaks the loop and prints "Goodbye.".
    writer.write_all(&[0x04]).expect("write EOT to pty");
    writer.flush().ok();

    let (saw_goodbye, captured) = wait_for(&rx, "Goodbye.", Duration::from_secs(10));
    assert!(
        saw_goodbye,
        "expected a clean exit ('Goodbye.') after Ctrl-D; captured so far:\n{captured}"
    );

    let status = wait_child(&mut child, Duration::from_secs(10));
    assert!(
        status.success(),
        "fleet must exit 0 after a clean interactive session; got {status:?}"
    );
}

/// `Child::wait()` blocks indefinitely on a hang; poll `try_wait()` instead so a regression that
/// makes the REPL hang after Ctrl-D fails this test instead of the whole suite.
fn wait_child(child: &mut std::process::Child, timeout: Duration) -> std::process::ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("fleet did not exit within {timeout:?} after Ctrl-D");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
