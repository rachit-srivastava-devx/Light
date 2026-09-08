//! One HTTP attempt against the llm7 endpoint. Split out of `client.rs` so the retry loop
//! there stays readable; this module knows nothing about retrying.

use crate::errors::ModelError;
use crate::llm7::parse::parse_response;
use crate::types::RawVerdict;
use serde_json::Value;

/// `Ok` on 2xx (parsed), `Err(RateLimited)` on 429 (with a hint for how long to wait), `Err`
/// otherwise for any other transport/HTTP failure.
pub enum AttemptError {
    RateLimited { retry_after_secs: u64 },
    Other(ModelError),
}

pub fn attempt(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    body: &Value,
) -> Result<RawVerdict, AttemptError> {
    let response = client
        .post(endpoint)
        .json(body)
        .send()
        .map_err(|e| AttemptError::Other(ModelError::new(format!("request failed: {e}"))))?;
    let status = response.status();
    let json: Value = response
        .json()
        .map_err(|e| AttemptError::Other(ModelError::new(format!("body not JSON: {e}"))))?;
    if status.as_u16() == 429 {
        let retry_after_secs = json
            .pointer("/error/retry_after")
            .and_then(Value::as_u64)
            .unwrap_or(5);
        return Err(AttemptError::RateLimited { retry_after_secs });
    }
    if !status.is_success() {
        return Err(AttemptError::Other(ModelError::new(format!(
            "http {status}: {json}"
        ))));
    }
    parse_response(&json).map_err(AttemptError::Other)
}
