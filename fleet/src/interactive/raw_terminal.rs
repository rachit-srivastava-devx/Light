//! Terminal raw mode guard and window sizing via libc for macOS.

use std::io;

pub struct RawModeGuard {
    orig: libc::termios,
}

impl RawModeGuard {
    pub fn enter() -> io::Result<Self> {
        let fd = libc::STDIN_FILENO;
        let mut termios: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut termios) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let orig = termios;
        unsafe { libc::cfmakeraw(&mut termios) };
        if unsafe { libc::tcsetattr(fd, libc::TCSADRAIN, &termios) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { orig })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSADRAIN, &self.orig);
        }
    }
}

#[allow(dead_code)]
pub fn terminal_size() -> (usize, usize) {
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    if unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) } == 0 && ws.ws_col > 0 {
        (ws.ws_col as usize, ws.ws_row as usize)
    } else {
        (80, 24)
    }
}
