use crate::VerifyError;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

pub(super) fn has_head(repo: &Path, budget: Duration) -> Result<bool, VerifyError> {
    let mut command = Command::new("git");
    command
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    super::process::io::scrub_environment(&mut command);
    let child = command
        .spawn()
        .map_err(|error| VerifyError::ScannerScopeUnavailable(error.to_string()))?;
    let output = super::process::io::bounded_output(child, budget)
        .map_err(|error| VerifyError::ScannerScopeUnavailable(error.to_string()))?;
    Ok(output.status.success())
}

pub(super) fn commit_count(repo: &Path, budget: Duration) -> Result<u64, VerifyError> {
    let mut command = Command::new("git");
    command
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    super::process::io::scrub_environment(&mut command);
    let child = command
        .spawn()
        .map_err(|error| VerifyError::ScannerScopeUnavailable(error.to_string()))?;
    let output = super::process::io::bounded_output(child, budget)
        .map_err(|error| VerifyError::ScannerScopeUnavailable(error.to_string()))?;
    if !output.status.success() {
        return Err(VerifyError::ScannerScopeUnavailable(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .map_err(|error| VerifyError::ScannerScopeUnavailable(error.to_string()))
}
