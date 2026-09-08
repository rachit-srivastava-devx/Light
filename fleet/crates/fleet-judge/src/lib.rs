//! `fleet-judge` -- LLM-as-a-judge: a pure scoring core (`judge`) over a caller-owned
//! `JudgeModel` port, plus a real keyless HTTP adapter (feature `llm7`). See `README.md`.

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
