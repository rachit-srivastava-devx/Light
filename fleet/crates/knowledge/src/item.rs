use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Scope(pub String);

impl Scope {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Lesson,
    Standard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeItem {
    pub id: String,
    pub kind: Kind,
    pub text_ref: String,
    pub scope: Scope,
    pub source_digest: String,
    pub evidence_count: u32,
    pub expires_at: Option<String>,
    pub revision: u64,
}

#[derive(Debug)]
pub struct SourceManifest {
    pub checked: u32,
    pub total: u32,
    pub items: Vec<KnowledgeItem>,
}
