//! `Llm7Judge`: the real `JudgeModel` adapter, blocking HTTP over the keyless llm7 endpoint.
//! Retries on 429s, honouring the server's `retry_after` (capped), up to `MAX_ATTEMPTS` --
//! the free endpoint shares a rate limit across callers and a burst of calls trips it.

use crate::errors::ModelError;
use crate::llm7::attempt::{attempt, AttemptError};
use crate::llm7::{schema::build_request, DEFAULT_ENDPOINT};
use crate::model::JudgeModel;
use crate::types::{Candidate, Criteria, RawVerdict};
use std::time::Duration;

const MAX_WAIT_SECS: u64 = 30;
const MAX_ATTEMPTS: u32 = 4;

pub struct Llm7Judge {
    endpoint: String,
    client: reqwest::blocking::Client,
}

impl Llm7Judge {
    pub fn new() -> Self {
        Self::with_endpoint(DEFAULT_ENDPOINT)
    }

    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl Default for Llm7Judge {
    fn default() -> Self {
        Self::new()
    }
}

impl JudgeModel for Llm7Judge {
    fn call(&self, criteria: &Criteria, candidate: &Candidate) -> Result<RawVerdict, ModelError> {
        let body = build_request(criteria, candidate);
        let mut last_wait = 0;
        for _ in 0..MAX_ATTEMPTS {
            match attempt(&self.client, &self.endpoint, &body) {
                Ok(v) => return Ok(v),
                Err(AttemptError::Other(e)) => return Err(e),
                Err(AttemptError::RateLimited { retry_after_secs }) => {
                    last_wait = retry_after_secs;
                    std::thread::sleep(Duration::from_secs(retry_after_secs.min(MAX_WAIT_SECS)));
                }
            }
        }
        Err(ModelError::new(format!(
            "still rate-limited after {MAX_ATTEMPTS} attempts, last wait hint {last_wait}s"
        )))
    }
}
