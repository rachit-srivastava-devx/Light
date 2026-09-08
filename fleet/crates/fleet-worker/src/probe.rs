//! The M1 keyless-thesis experiment (§12's highest-risk unknown): drive `adapter` with every
//! ambient credential source removed and report whether it stayed cleanly blocked or produced
//! an unexplained success -- the caller (a human, per §12) must inspect `evidence_paths` by hand
//! for `UnexpectedSuccess`; no test in this crate can fully automate that classification.

use crate::outcome::{LaneOutcome, NoAmbientProbeResult};
use crate::request::{SpawnError, SpawnRequest};
use crate::{join, spawn, CliAdapter, Role};
use fleet_types::TaskId;
use std::path::Path;
use std::time::Duration;

pub fn probe_no_ambient(adapter: CliAdapter, repo: &Path) -> Result<NoAmbientProbeResult, SpawnError> {
    let request = SpawnRequest {
        repo: repo.to_path_buf(),
        role: Role::Builder,
        task_id: TaskId::parse("probe-no-ambient").expect("static id is non-empty"),
        adapter,
        requested_model: None,
        task: "probe: report your own environment, do nothing else".to_string(),
        deadline: Duration::from_secs(30),
    };
    let handle = spawn(request)?;
    let sandbox_evidence = handle.sandbox_root.clone();
    let outcome = join(handle).map_err(|e| SpawnError::ProcessSpawnFailed(e.to_string()))?;
    Ok(match outcome {
        LaneOutcome::EnvironmentFault { detail } => NoAmbientProbeResult::CleanlyBlocked { detail },
        LaneOutcome::Refused { reason } => NoAmbientProbeResult::CleanlyBlocked { detail: reason },
        LaneOutcome::Done { body, .. } => NoAmbientProbeResult::UnexpectedSuccess {
            detail: format!("adapter {adapter:?} produced Done with no ambient credentials: {body}"),
            evidence_paths: vec![sandbox_evidence],
        },
    })
}
