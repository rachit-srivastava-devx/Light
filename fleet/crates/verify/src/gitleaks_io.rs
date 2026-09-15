use crate::VerifyError;
use std::process::{Command, Output};
use std::time::Duration;

use super::wait;
use super::{env, stream};

pub(crate) fn scrub_environment(command: &mut Command) {
    env::scrub(command);
}

pub(crate) fn bounded_output(
    mut child: std::process::Child,
    budget: Duration,
) -> Result<Output, VerifyError> {
    let stdout = child.stdout.take().map(stream::drain_stdout);
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| VerifyError::ScannerUnavailable("scanner stderr was not piped".into()))?;
    let reader = stream::drain_stderr(stderr);
    let result = wait::bounded_status(child, budget);
    let captured = match reader.join() {
        Ok(bytes) => bytes,
        Err(_) => {
            return Err(VerifyError::ScannerUnavailable(
                "stderr reader failed".into(),
            ))
        }
    };
    let stdout = stdout
        .map(|reader| {
            reader
                .join()
                .map_err(|_| VerifyError::ScannerUnavailable("stdout reader failed".into()))
        })
        .transpose()?
        .unwrap_or_default();
    Ok(Output {
        status: result?,
        stdout,
        stderr: captured,
    })
}
