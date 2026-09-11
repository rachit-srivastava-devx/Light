//! Key event parsing for terminal raw mode.

use std::io::{self, Read};

#[derive(Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Tab,
    ShiftTab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Delete,
    CtrlC,
    CtrlD,
    CtrlL,
    Unknown,
}

pub fn read_key() -> io::Result<Key> {
    let mut buf = [0u8; 1];
    let n = io::stdin().read(&mut buf)?;
    if n == 0 {
        return Ok(Key::CtrlD);
    }
    match buf[0] {
        b'\r' | b'\n' => Ok(Key::Enter),
        127 | 8 => Ok(Key::Backspace),
        b'\t' => Ok(Key::Tab),
        3 => Ok(Key::CtrlC),
        4 => Ok(Key::CtrlD),
        12 => Ok(Key::CtrlL),
        0x1b => parse_escape(),
        b if b >= 32 => Ok(Key::Char(b as char)),
        _ => Ok(Key::Unknown),
    }
}

fn parse_escape() -> io::Result<Key> {
    let mut seq = [0u8; 2];
    if io::stdin().read(&mut seq[..1])? == 0 {
        return Ok(Key::Unknown);
    }
    if seq[0] == b'[' {
        if io::stdin().read(&mut seq[1..2])? == 0 {
            return Ok(Key::Unknown);
        }
        match seq[1] {
            b'A' => Ok(Key::Up),
            b'B' => Ok(Key::Down),
            b'C' => Ok(Key::Right),
            b'D' => Ok(Key::Left),
            b'H' => Ok(Key::Home),
            b'F' => Ok(Key::End),
            b'Z' => Ok(Key::ShiftTab),
            b'3' => {
                let mut tilde = [0u8; 1];
                let _ = io::stdin().read(&mut tilde);
                Ok(Key::Delete)
            }
            _ => Ok(Key::Unknown),
        }
    } else {
        Ok(Key::Unknown)
    }
}
