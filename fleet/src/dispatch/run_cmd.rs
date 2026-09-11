//! `fleet run` and the hidden `__pipeline_probe`: parse -> `pipeline::run_pipeline` -> print.
//! This is the one place the CLI layer drives the durable pipeline graph end to end.

use crate::cli::args_ops::RunArgs;
use crate::dispatch::error::DispatchError;
use crate::pipeline::{maybe_flush, run_pipeline};
use crate::print::json;
use fleet_types::TaskId;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Every candidate confirmed installed, with an unmeasured-but-generous quota window and no
/// cooldown -- the "everything is healthy" default a fresh CLI invocation has no better basis to
/// assume (real measured values are `fleet-govern`'s job, not this composition root's).
fn healthy_runtime() -> fleet_router::RuntimeState {
    let preference: Vec<&'static str> = fleet_router::ORDER.iter().map(|c| c.id).collect();
    let capable = fleet_router::ORDER.iter().map(|c| c.adapter).collect::<BTreeSet<_>>();
    let remaining = capable.iter().map(|a| (a.to_string(), Some(u64::MAX))).collect::<BTreeMap<_, _>>();
    fleet_router::RuntimeState { capable, remaining, cooldown: BTreeSet::new(), required_tokens: 0, preference }
}

pub fn run(state_dir: &Path, args: RunArgs) -> Result<(), DispatchError> {
    run_pipeline_on(state_dir, args.repo, args.task, args.json)
}

/// Shared entrypoint for the verify pipeline. `run` is the CLI-invoked path; `swarm_cmd`'s
/// `--then-verify` reuses this same function so the two never drift (BLUEPRINT §5: one
/// composition root per subcommand -- verify's one place is here, not duplicated in swarm).
/// `json` mirrors `RunArgs::json` so callers can request either the machine or human report.
pub(crate) fn run_pipeline_on(
    state_dir: &Path,
    repo: String,
    task_id: String,
    json: bool,
) -> Result<(), DispatchError> {
    let task = TaskId::parse(task_id).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let repo = PathBuf::from(&repo);
    // The committed table with this repo's `.fleet/gates.toml` applied -- identical to
    // `fleet_verify::GATES` when the repo has no such file (see `gate_config`).
    let gates = super::gate_config::resolve(&repo)?;
    let outcome = run_pipeline(state_dir, &repo, task, &healthy_runtime(), &gates);
    maybe_flush(state_dir);
    if json {
        json::print_pretty(&outcome);
    } else {
        crate::print::run_report::render(&outcome);
    }
    match outcome.result {
        Ok(()) => Ok(()),
        Err(e) => Err(DispatchError::Refusal(e.to_string())),
    }
}

/// `__pipeline_probe` runs `Verify` against ZERO gates, still through the real
/// `run_all`/`WhichProbe`/`RealRunner` path -- not `fleet_verify::GATES`. This machine has every
/// gate's tool on `PATH` (`cargo`, `cargo-mutants`, `semgrep`, `trivy`, `conftest`, `uv`, `bash`),
/// so the real table would actually execute `cargo test --workspace` (this probe is itself
/// invoked BY that gate's own integration-test run -- unbounded recursion) and `cargo mutants`
/// (minutes-to-hours per run) inside what must stay a fast, deterministic crash-resume test.
/// `fleet run` never filters: it always gets the real, full table (`gate_config::resolve`).
const NO_GATES: &[fleet_verify::GateSpec] = &[];

pub fn pipeline_probe(state_dir: &Path, repo: String, task_id: String) -> Result<(), DispatchError> {
    let task = TaskId::parse(task_id).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let outcome = run_pipeline(state_dir, Path::new(&repo), task, &healthy_runtime(), NO_GATES);
    maybe_flush(state_dir);
    json::print_pretty(&outcome);
    outcome.result.map_err(|e| DispatchError::Refusal(e.to_string()))
}
