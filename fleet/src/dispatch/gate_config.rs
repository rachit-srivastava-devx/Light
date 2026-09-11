//! Per-repo gate commands: the committed `fleet_verify::GATES` table with `.fleet/gates.toml`'s
//! overrides applied, so `fleet run|gate|oracle` can act as a quality gate for a repo that is not
//! a cargo workspace. The unit-tests gate shelled out to `cargo test --workspace` unconditionally,
//! so on a Node repo it SKIPped as "unavailable" -- five of eight gates never meaningfully ran.
//!
//! `GATES` is already a parameter of `run_all`/`run_pipeline`, so nothing in `fleet-verify`
//! changes: this builds a `Vec<GateSpec>` at the CLI layer and hands it to the same call sites.
//! With no config file the result is `GATES.to_vec()` -- byte-identical behaviour.
//!
//! A repo may only REPLACE a known gate's command (and the tool probed for it). It may not add,
//! remove, or downgrade a gate, and an id that is not in the registry is an error rather than a
//! no-op -- a typo'd id must not silently leave the gate running `cargo test`.

use super::error::DispatchError;
use super::gate_config_file as file;
use fleet_verify::{GateCommand, GateSpec, ProbeTool};
use std::path::Path;

/// `GateSpec`'s `id`/`command`/`probe` are all `&'static` (the type is `Copy`, built as a `const`
/// table), so a command read from a file has to outlive the process to go in one. The leak is
/// bounded and one-shot: at most one config file per invocation, a handful of gates each. Same
/// idiom the crate's own `run_all` acceptance test uses for a generated gate id.
fn leak(text: &str) -> &'static str {
    Box::leak(text.to_string().into_boxed_str())
}

fn leak_argv(argv: &[String]) -> &'static [&'static str] {
    Box::leak(argv.iter().map(|a| leak(a)).collect::<Vec<_>>().into_boxed_slice())
}

fn known_ids() -> String {
    fleet_verify::GATES.iter().map(|g| format!("{:?}", g.id)).collect::<Vec<_>>().join(", ")
}

fn fault(reason: String) -> DispatchError {
    DispatchError::EnvFault(format!("{}: {reason}", file::RELATIVE))
}

/// The gate table to run against `repo`. Absent config -> the committed table, unchanged.
pub fn resolve(repo: &Path) -> Result<Vec<GateSpec>, DispatchError> {
    let mut specs: Vec<GateSpec> = fleet_verify::GATES.to_vec();
    let Some(config) = file::load(repo).map_err(DispatchError::EnvFault)? else {
        return Ok(specs);
    };
    for (id, over) in &config.gates {
        if over.command.is_empty() {
            return Err(fault(format!("gate {id:?} has an empty command")));
        }
        let Some(spec) = specs.iter_mut().find(|s| s.id == id) else {
            return Err(fault(format!("gate {id:?} is not in the registry (known: {})", known_ids())));
        };
        spec.command = GateCommand::OnPath(leak_argv(&over.command));
        if let Some(probe) = &over.probe {
            spec.probe = ProbeTool::Named(leak(probe));
        }
    }
    Ok(specs)
}

#[cfg(test)]
#[path = "gate_config_tests.rs"]
mod tests;
