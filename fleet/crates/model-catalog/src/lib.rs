pub mod discover;
pub mod probe;
pub mod qualification;

pub use discover::{discover, DiscoveryConfig};
pub use probe::probe;
pub use qualification::qualify;

use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CapabilityState {
    Supported,
    Unsupported,
    Unknown { reason: String },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Candidate {
    pub path: std::path::PathBuf,
    pub version: String,
    pub digest: String,
}

#[derive(Clone, Debug)]
pub struct CapabilityReport {
    pub candidate: Candidate,
    pub capabilities: HashMap<String, CapabilityState>,
    pub timestamp: std::time::SystemTime,
}

#[derive(Clone, Debug)]
pub struct CatalogSnapshot {
    pub records: Vec<CapabilityReport>,
    pub digest: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("insufficient trials: need at least 1, got {got}")]
    InsufficientTrials { got: u32 },
    #[error("candidate not found: {path}")]
    NotFound { path: String },
    #[error("probe error: {0}")]
    ProbeError(String),
}
