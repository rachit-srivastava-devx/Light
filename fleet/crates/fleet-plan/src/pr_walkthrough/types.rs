//! The `PrWalkthrough` shape (ask #19) plus its plain caller-supplied inputs -- this crate never
//! reads a diff, an acceptance runner, or an attestation off disk; the composition root resolves
//! those and hands in these already-summarised values.

/// One changed file plus its line delta -- summarised, never the raw diff text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileChange {
    pub path: String,
    pub lines_added: u32,
    pub lines_removed: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffSummary {
    pub files: Vec<FileChange>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptanceResult {
    pub check_name: String,
    pub oracle_kind: String,
    pub passed: bool,
    pub detail: String,
}

/// What the real `fleet_types::Attestation` resolves to for this PR, reduced to the fields a
/// walkthrough narrates -- the composition root extracts these from the full attestation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttestationSummary {
    pub builder: String,
    pub verdict: crate::review::VerdictName,
    pub notes: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedItem {
    pub check_name: String,
    pub detail: String,
}

/// What changed and why, the one riskiest part, what was verified vs not, and what a reviewer
/// should check first -- built once a change lands, before a human reviews it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrWalkthrough {
    pub what_changed: String,
    pub why: String,
    pub riskiest_part: String,
    pub verified: Vec<VerifiedItem>,
    pub not_verified: Vec<VerifiedItem>,
    pub reviewer_focus: Vec<String>,
}
