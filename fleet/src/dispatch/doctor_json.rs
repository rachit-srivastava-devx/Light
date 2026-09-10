//! `fleet doctor --json`'s report shape -- split out of `ops_cmd.rs` to keep it ≤80 lines.

use crate::build_info::BuildIdentity;
use crate::runtime::capacity::{preflight, PreflightConfig, StdCapacityProbe};

#[derive(serde::Serialize)]
pub struct DoctorReport {
    pub cargo: bool,
    /// Where cargo was found, or every directory searched -- the boolean above alone sent a user
    /// with cargo in `~/.cargo/bin` (off `$PATH`) off to reinstall a tool they already had.
    pub cargo_path: String,
    pub git: bool,
    pub git_path: String,
    pub capacity_decision: String,
    #[serde(flatten)]
    pub build: BuildIdentity,
}

/// Whether a tool resolves, and where -- `super::tool_path` searches `$PATH` then rustup's dirs.
pub fn probe(tool: &str) -> (bool, String) {
    match super::tool_path::find(tool) {
        Some(p) => (true, p.display().to_string()),
        None => (false, format!("missing (searched {})", super::tool_path::searched())),
    }
}

pub fn build(cargo: bool, cargo_path: String, git: bool, git_path: String) -> DoctorReport {
    let cfg = PreflightConfig::from_env(3, None);
    let capacity_decision = match preflight(&StdCapacityProbe, &cfg) {
        Ok(cap) => format!("allow (concurrency_cap={})", cap.get()),
        Err(refusal) => format!("refuse: {refusal}"),
    };
    let build = crate::build_info::IDENTITY;
    DoctorReport { cargo, cargo_path, git, git_path, capacity_decision, build }
}
