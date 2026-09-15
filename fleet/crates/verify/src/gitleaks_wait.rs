use crate::VerifyError;
use std::process::Child;
#[cfg(test)]
use std::process::Output;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(test)]
pub(super) fn bounded(mut child: Child, budget: Duration) -> Result<Output, VerifyError> {
    let deadline = Instant::now() + budget;
    let mut pause = Duration::from_millis(10);
    loop {
        match child
            .try_wait()
            .map_err(|error| VerifyError::ScannerUnavailable(error.to_string()))?
        {
            Some(_) => {
                return child
                    .wait_with_output()
                    .map_err(|error| VerifyError::ScannerUnavailable(error.to_string()))
            }
            None if Instant::now() >= deadline => {
                terminate(&mut child);
                let _ = child.wait();
                return Err(VerifyError::ScannerTimeout);
            }
            None => {
                thread::sleep(pause);
                pause = if pause < Duration::from_millis(250) {
                    pause + pause
                } else {
                    pause
                };
            }
        }
    }
}

pub(super) fn bounded_status(
    mut child: Child,
    budget: Duration,
) -> Result<std::process::ExitStatus, VerifyError> {
    let deadline = Instant::now() + budget;
    let mut pause = Duration::from_millis(10);
    loop {
        match child
            .try_wait()
            .map_err(|error| VerifyError::ScannerUnavailable(error.to_string()))?
        {
            Some(status) => return Ok(status),
            None if Instant::now() >= deadline => {
                terminate(&mut child);
                let _ = child.wait();
                return Err(VerifyError::ScannerTimeout);
            }
            None => {
                thread::sleep(pause);
                pause = if pause < Duration::from_millis(250) {
                    pause + pause
                } else {
                    pause
                };
            }
        }
    }
}

fn terminate(child: &mut Child) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
}

#[cfg(test)]
#[path = "gitleaks_tests.rs"]
mod tests;
