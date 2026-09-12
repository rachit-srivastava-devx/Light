//! LLD-ready gate types + the 14-check table. Verbatim from `lld_ready.rs:21-105`.

use serde_json::Value;

/// The 14 check ids, byte-identical to and in the same order as `lld_ready.rs:24-39`.
pub const GATE_CHECK_IDS: [&str; 14] = [
    "C1-OPEN", "C2-OWNER", "C3-ACC-PARSE", "C3-ACC-GROUND", "C3-ACC-NONTAUT", "R17-DERIV",
    "R19-ABSOLUTE", "R21-ALTS", "R21-FAIL", "C12-STORE", "C12-DEPS", "REG-VERDICT", "IFACE",
    "SHAPE",
];

/// Caller-supplied reference sets. NOT read from disk here -- the composition root reads
/// `owners.v1.json`/`gate-refs.v1.json` and constructs this.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GateRefs {
    pub owners: Vec<String>,
    pub registry_paths: Vec<String>,
    pub known_node_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateReason {
    pub check_id: &'static str,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DepthScore {
    pub checks_total: u32,
    pub checks_passed: u32,
    /// `round(passed/total, 3)` -- report data, never a decision input.
    pub ratio: f64,
    pub failed_check_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ready,
    NotReady,
    MeasuredNothing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub outcome: Outcome,
    pub checked: usize,
    /// `None` **iff** `outcome == MeasuredNothing`.
    pub score: Option<DepthScore>,
    pub reasons: Vec<GateReason>,
}

pub type Check = (&'static str, fn(&Value, &GateRefs) -> bool);

/// The gate's identity. THE ONLY place in this crate that constructs this literal for writing.
pub const STAMPED_BY: &str = "keel:lld-ready";
