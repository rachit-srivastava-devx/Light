use serde::{Deserialize, Serialize};

/// All seven fixed boolean predicates plus denomination and digest fields.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadyInput {
    pub acceptance: Vec<String>,
    pub questions_open: u64,
    pub reviewer_accepts: bool,
    pub deps_pinned: bool,
    pub grants_cover: bool,
    pub write_scope_exclusive: bool,
    pub resources_available: bool,
    pub resource_profile: String,
    pub checked: u64,
    pub total: u64,
    /// Digest the reviewer signed against (for staleness check).
    pub plan_digest: String,
    /// Digest the reviewer produced (must equal plan_digest).
    pub reviewer_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Status {
    Ready,
    NotReady,
    ZeroCoverage,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Violation {
    EmptyAcceptance,
    OpenQuestions(u64),
    ReviewerNotAccepted,
    DepsNotPinned,
    GrantsNotCovered,
    WriteNotExclusive,
    ResourcesUnavailable,
    StaleDigest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadyVerdict {
    pub status: Status,
    pub violations: Vec<Violation>,
    pub checked: u64,
    pub total: u64,
}
