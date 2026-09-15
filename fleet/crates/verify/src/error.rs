use thiserror::Error;

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("no gates specified")]
    NoGates,
    #[error("gate invalid: {0}")]
    InvalidGate(String),
    #[error("candidate invalid: {0}")]
    InvalidCandidate(String),
    #[error("coverage error: {0}")]
    CoverageParseError(String),
    #[error("secret found")]
    SecretFound,
    #[error("secret scanner unavailable: {0}")]
    ScannerUnavailable(String),
    #[error("secret scanner failed (exit={code:?}): {stderr}")]
    ScannerFailed { code: Option<i32>, stderr: String },
    #[error("secret scanner timed out")]
    ScannerTimeout,
    #[error("secret scanner output invalid: {0}")]
    ScannerParse(String),
    #[error("secret scanner scope unavailable: {0}")]
    ScannerScopeUnavailable(String),
}
