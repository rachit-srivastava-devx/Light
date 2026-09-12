//! Ask #19 -- "teach the PR it created to the user in detail": a deterministic, structured
//! outline of a finished change built purely from real change data (diff, module brief,
//! acceptance results, attestation). No model call lives here.

mod build;
mod error;
mod risk;
mod types;

pub use build::build_pr_walkthrough;
pub use error::PrWalkthroughError;
pub use types::{AttestationSummary, DiffSummary, FileChange, PrWalkthrough, VerifiedItem};
pub use types::AcceptanceResult;
