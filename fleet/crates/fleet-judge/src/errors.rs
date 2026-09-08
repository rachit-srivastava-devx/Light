//! Typed errors for the judge. No `unwrap()`, no silent defaulting: a call either produces a
//! `Verdict` or one of these, never a best-effort guess.

/// The injected `JudgeModel` port failed to produce any reply (network, HTTP status, transport
/// decode). Adapter-specific detail lives in `reason`; this crate's pure core never inspects it.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("judge model call failed: {reason}")]
pub struct ModelError {
    pub reason: String,
}

impl ModelError {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// Everything that can go wrong in `judge`.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum JudgeError {
    #[error(transparent)]
    Model(#[from] ModelError),

    /// `Criteria::labels` was empty -- there is nothing to judge among.
    #[error("criteria has no labels to choose among")]
    EmptyLabelSet,

    /// The model replied, but not with a well-formed decision or abstention: e.g. no label and
    /// no abstain reason, a label outside `Criteria::labels`, or an out-of-range confidence.
    #[error("model returned a malformed verdict: {0}")]
    MalformedResponse(String),
}
