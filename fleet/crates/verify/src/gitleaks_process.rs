use crate::VerifyError;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

#[path = "gitleaks_env.rs"]
mod env;
#[path = "gitleaks_io.rs"]
pub(crate) mod io;
#[path = "gitleaks_stream.rs"]
mod stream;
#[path = "gitleaks_wait.rs"]
mod wait;

pub(super) fn run(
    binary: &Path,
    repo: &Path,
    report: &Path,
    budget: Duration,
    git_required: bool,
) -> Result<Output, VerifyError> {
    let report = report
        .to_str()
        .ok_or_else(|| VerifyError::ScannerUnavailable("non-UTF8 report path".into()))?;
    let repo_text = repo
        .to_str()
        .ok_or_else(|| VerifyError::ScannerUnavailable("non-UTF8 repository path".into()))?;
    let mut command = Command::new(binary);
    command
        .args(if git_required {
            vec!["git", "--no-banner"]
        } else {
            vec!["dir", "--no-banner", repo_text]
        })
        .args([
            "--redact",
            "--report-format",
            "json",
            "--report-path",
            report,
            "--exit-code",
            "0",
        ])
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    io::scrub_environment(&mut command);
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let child = command
        .spawn()
        .map_err(|error| VerifyError::ScannerUnavailable(error.to_string()))?;
    io::bounded_output(child, budget)
}

pub(super) fn version(binary: &Path, budget: Duration) -> Result<Output, VerifyError> {
    let mut command = Command::new(binary);
    command
        .arg("version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    io::scrub_environment(&mut command);
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let child = command
        .spawn()
        .map_err(|error| VerifyError::ScannerUnavailable(error.to_string()))?;
    io::bounded_output(child, budget)
}

pub(super) fn diagnostic(bytes: &[u8]) -> String {
    const MAX: usize = 8 * 1024;
    String::from_utf8_lossy(&bytes[..bytes.len().min(MAX)])
        .trim()
        .to_string()
}
