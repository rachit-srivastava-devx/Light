
mod errors;
mod judge;
mod model;
mod types;
mod verdict;

#[cfg(feature = "llm7")]
pub mod llm7;

pub use errors::{JudgeError, ModelError};
pub use judge::judge;
pub use model::JudgeModel;
pub use types::{Candidate, Criteria, RawVerdict};
pub use verdict::Verdict;
