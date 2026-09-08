//! `fleet doctor --json`'s report shape -- split out of `ops_cmd.rs` to keep it ≤80 lines.

use crate::runtime::capacity::{preflight, PreflightConfig, StdCapacityProbe};

#[derive(serde::Serialize)]
pub struct DoctorReport {
    pub cargo: bool,
    pub git: bool,
    pub capacity_decision: String,
}

pub fn build(cargo: bool, git: bool) -> DoctorReport {
    let cfg = PreflightConfig::from_env(3, None);
    let capacity_decision = match preflight(&StdCapacityProbe, &cfg) {
        Ok(cap) => format!("allow (concurrency_cap={})", cap.get()),
        Err(refusal) => format!("refuse: {refusal}"),
    };
    DoctorReport { cargo, git, capacity_decision }
}
