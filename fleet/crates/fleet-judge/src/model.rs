//! `JudgeModel` -- the port `fleet-judge`'s pure core calls through. Mirrors
//! `fleet-context::ConventionFs` / `fleet-memory::PatternMatcher`: the trait lives in the
//! consuming crate, concrete adapters (HTTP, fake) implement it, and the pure core never names
//! a transport type.

use crate::errors::ModelError;
use crate::types::{Candidate, Criteria, RawVerdict};

/// One model call: given a rubric and a candidate, return the model's raw (unvalidated) reply.
/// Implementations own all I/O (HTTP, retries, timeouts); `judge` treats this as a single
/// opaque step and validates the result itself.
pub trait JudgeModel {
    fn call(&self, criteria: &Criteria, candidate: &Candidate) -> Result<RawVerdict, ModelError>;
}
