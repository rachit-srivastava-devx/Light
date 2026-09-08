//! `Verdict` -- the judge's typed output. There is no "unknown"/`Option::None` case: a judge
//! that cannot decide must say so via `Abstain`, never silently pick a label or default one in.

/// The judge's decision on one `Candidate` against one `Criteria`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// The judge chose `label` (a member of the `Criteria::labels` it was given).
    Decided {
        label: String,
        confidence_pct: u8,
        because: String,
    },
    /// The judge explicitly declined to choose. Distinct from an error: the model call
    /// succeeded and answered, and its answer was "I can't tell."
    Abstain { why: String },
}

impl Verdict {
    /// The chosen label, or `None` for an abstention.
    pub fn label(&self) -> Option<&str> {
        match self {
            Verdict::Decided { label, .. } => Some(label),
            Verdict::Abstain { .. } => None,
        }
    }
}
