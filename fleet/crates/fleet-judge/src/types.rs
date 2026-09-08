//! Inputs to `judge`: the rubric (`Criteria`) and the thing being judged (`Candidate`), plus
//! `RawVerdict` -- the un-validated shape a `JudgeModel` port hands back. Validation into a
//! typed `Verdict` happens in `judge` (pure), never inside a model adapter.

/// A judging rubric: free-text instructions plus the closed set of labels the judge may choose
/// among. `labels` must be non-empty for `judge` to accept it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Criteria {
    pub instructions: String,
    pub labels: Vec<String>,
}

/// The thing being judged: one opaque input string scored against `Criteria`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub input: String,
}

/// What a `JudgeModel` port call returns, before `judge` validates it. Every field is optional
/// because the model may reply with a decision, an abstention, or (on a malformed tool call)
/// neither -- `judge` is what turns that ambiguity into a typed `Verdict` or `JudgeError`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RawVerdict {
    pub label: Option<String>,
    pub confidence_pct: Option<u8>,
    pub because: Option<String>,
    pub abstain_why: Option<String>,
}
