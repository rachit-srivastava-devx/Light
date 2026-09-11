//! Raw-mode line reader supporting cursor editing, history, and Shift+Tab.

use super::keys::{read_key, Key};
use super::raw_terminal::RawModeGuard;
use super::theme::*;
use std::io::{self, Write};

pub enum ReadOutcome {
    Submit(String),
    ToggleMode,
    Clear,
    Exit,
}

pub fn read_input(prompt: &str, color: bool, history: &[String]) -> ReadOutcome {
    let _guard = match RawModeGuard::enter() {
        Ok(g) => g,
        Err(_) => return fallback_read(),
    };
    let mut buf = String::new();
    let mut cursor = 0usize;
    let mut hist_idx = history.len();

    loop {
        render_line(prompt, &buf, cursor, color);
        match read_key().unwrap_or(Key::CtrlC) {
            Key::Char(c) => { buf.insert(cursor, c); cursor += 1; }
            Key::Backspace if cursor > 0 => { cursor -= 1; buf.remove(cursor); }
            Key::Left if cursor > 0 => cursor -= 1,
            Key::Right if cursor < buf.len() => cursor += 1,
            Key::Home => cursor = 0,
            Key::End => cursor = buf.len(),
            Key::ShiftTab => return ReadOutcome::ToggleMode,
            Key::CtrlC | Key::CtrlD => { println!(); return ReadOutcome::Exit; }
            Key::CtrlL => return ReadOutcome::Clear,
            Key::Up if hist_idx > 0 => {
                hist_idx -= 1;
                buf = history[hist_idx].clone();
                cursor = buf.len();
            }
            Key::Down => {
                if hist_idx + 1 < history.len() {
                    hist_idx += 1;
                    buf = history[hist_idx].clone();
                } else {
                    hist_idx = history.len();
                    buf.clear();
                }
                cursor = buf.len();
            }
            Key::Enter => { println!(); return ReadOutcome::Submit(buf); }
            _ => {}
        }
    }
}

fn render_line(prompt: &str, buf: &str, cursor: usize, color: bool) {
    let p = paint(color, BOLD, prompt);
    let before = &buf[..cursor];
    print!("\r\x1b[K{p}{buf}\r\x1b[K{p}{before}");
    let _ = io::stdout().flush();
}

fn fallback_read() -> ReadOutcome {
    print!("> ");
    let _ = io::stdout().flush();
    let mut s = String::new();
    if io::stdin().read_line(&mut s).is_ok() {
        ReadOutcome::Submit(s.trim_end().into())
    } else {
        ReadOutcome::Exit
    }
}
