//! `fleet run` and the hidden `__pipeline_probe`: parse -> `pipeline::run_pipeline` -> print.
//! This is the one place the CLI layer drives the durable pipeline graph end to end.
//!
//! Multi-repo (`--repo a --repo b ...`): the per-repo work is factored into
//! `run_pipeline_on`, and `run` loops sequentially -- one task_id, N repos, one aggregated
//! verdict. Per-repo receipts land in the ledger under the same task_id (correlator), and the
//! final exit code is the worst of the set. Sequential is intentional (single-machine
//! concurrency cap; no per-repo parallelism in this PR). This mirrors the Frido workspace's
//! three sibling repos (posx-frido-{store,backend,admin}) where one feature touches 2-3 of them.

use crate::cli::args_ops::RunArgs;
use crate::dispatch::error::DispatchError;
use super::verify_repo::ensure_repo;
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

/// One repo's slice of the multi-repo run: resolve gates for THIS repo, drive the pipeline,
/// flush, render (human stderr summary or JSON on stdout), and return `Ok`/`Err` derived from
/// the pipeline's own `result`. Extracted from `run` so the multi-repo loop below is a plain
/// `for` over this fn (a small refactor -- PR #17's `feat/swarm-then-verify` extracts the same
/// seam for the swarm-then-verify path; a merge conflict here is expected and trivial).
pub(crate) fn run_pipeline_on(
    state_dir: &Path,
    repo: &Path,
    task: TaskId,
    json_out: bool,
) -> Result<(), DispatchError> {
    // The committed table with this repo's `.fleet/gates.toml` applied -- identical to
    // `fleet_verify::GATES` when the repo has no such file (see `gate_config`).
    let gates = super::gate_config::resolve(repo)?;
    let outcome = run_pipeline(state_dir, repo, task, &healthy_runtime(), &gates);
    maybe_flush(state_dir);
    if json_out {
        json::print_pretty(&outcome);
    } else {
        crate::print::run_report::render(&outcome);
    }
    match outcome.result {
        Ok(()) => Ok(()),
        Err(e) => Err(DispatchError::Refusal(e.to_string())),
    }
}

pub fn run(state_dir: &Path, args: RunArgs) -> Result<(), DispatchError> {
    let task = TaskId::parse(args.task).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    // Preflight EVERY --repo first (typed refusal on nonexistent/unreadable paths, per
    // `verify_repo::ensure_repo`). Refuse before running anything so a bad path in position 3
    // doesn't leave repos 1-2 with completed receipts and confuse the operator.
    let repos: Vec<PathBuf> =
        args.repos.iter().map(|r| ensure_repo(r)).collect::<Result<_, _>>()?;
    // Sequential loop -- single-machine concurrency cap governs per-repo work; per-PR-scope
    // this stays serial. Same task_id across all repos so the ledger correlates the set.
    let mut first_err: Option<DispatchError> = None;
    let mut passed = 0usize;
    for repo in &repos {
        let display = repo.display();
        match run_pipeline_on(state_dir, repo, task.clone(), args.json) {
            Ok(()) => {
                passed += 1;
                if !args.json {
                    eprintln!("PASS {display}");
                }
            }
            Err(e) => {
                if !args.json {
                    eprintln!("FAIL {display}: {e}");
                }
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
    }
    if !args.json && repos.len() > 1 {
        let total = repos.len();
        let failed = total - passed;
        eprintln!("rollup: {passed}/{total} passed, {failed} failed");
    }
    // Aggregated verdict: worst of the set. With one collected error, its exit code wins
    // (Refusal here; `main` maps that via `DispatchError::exit_code`). Multiple errors surface
    // via the per-repo FAIL lines above -- returning the first keeps `main`'s `eprintln!("fleet:
    // {e}")` from double-printing what the loop already reported.
    match first_err {
        Some(e) => Err(e),
        None => Ok(()),
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
