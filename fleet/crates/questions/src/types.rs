#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Question {
    pub text: String,
    pub why: String,
    pub severity: u8,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct QuestionSet {
    pub revision: u64,
    pub items: Vec<Question>,
}

#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("question has empty why")]
    EmptyWhy,
    #[error("question has empty text")]
    EmptyText,
    #[error("cap exceeds maximum of 3")]
    InvalidCap,
    #[error("invalid revision: must be nonzero")]
    InvalidRevision,
}
