use crate::VerifyError;
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

const EXPECTED_MAJOR: &str = "8.";

pub(super) fn resolve(configured: Option<&Path>, budget: Duration) -> Result<PathBuf, VerifyError> {
    let path = configured
        .map(PathBuf::from)
        .or_else(|| env::var_os("FLEET_GITLEAKS_BIN").map(PathBuf::from))
        .or_else(|| {
            env::split_paths(&env::var_os("PATH")?).find_map(|dir| {
                let candidate = dir.join("gitleaks");
                candidate.is_file().then_some(candidate)
            })
        })
        .ok_or_else(|| VerifyError::ScannerUnavailable("gitleaks not found".into()))?;
    let resolved = path
        .canonicalize()
        .map_err(|error| VerifyError::ScannerUnavailable(error.to_string()))?;
    if !resolved.is_file() {
        return Err(VerifyError::ScannerUnavailable(
            "gitleaks path is not a file".into(),
        ));
    }
    let version = super::process::version(&resolved, budget)?;
    if !version.status.success() {
        return Err(VerifyError::ScannerFailed {
            code: version.status.code(),
            stderr: super::process::diagnostic(&version.stderr),
        });
    }
    let version = String::from_utf8_lossy(&version.stdout);
    if !version
        .split_whitespace()
        .any(|word| word.starts_with(EXPECTED_MAJOR))
    {
        return Err(VerifyError::ScannerUnavailable(
            "unsupported gitleaks version (expected 8.x)".into(),
        ));
    }
    Ok(resolved)
}
