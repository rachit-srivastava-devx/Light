//! `fleet run-modules`'s arg parsing, split out of `mod.rs` to keep it under the 80-line gate.

use cli::args_core::RunModulesArgs;
use crate::dispatch::error::DispatchError;
use crate::dispatch::run_cmd;
use std::path::Path;
use types::Module;

/// Parse modules from CLI args. Neither `--modules` nor `--modules-file` has a parser wired up
/// yet, so this refuses rather than silently running the rest of `run-modules` against zero
/// modules (AGENTS.md hard rule 6: a gate/check that examined zero inputs must never pass).
pub fn parse_modules_from_args(args: &RunModulesArgs) -> Result<Vec<Module>, DispatchError> {
    if args.modules_file.is_some() {
        return Err(DispatchError::Refusal(
            "run-modules: --modules-file parsing is not implemented yet".to_string(),
        ));
    }
    if !args.modules.is_empty() {
        return Err(DispatchError::Refusal(
            "run-modules: --modules parsing is not implemented yet".to_string(),
        ));
    }
    Err(DispatchError::Refusal(
        "run-modules: no modules given (pass --modules or --modules-file)".to_string(),
    ))
}

/// Entry point for running multiple modules in parallel.
/// This is a convenience wrapper around `run_cmd::run_modules` that can be called from async context.
#[expect(dead_code)]
pub async fn run_parallel_modules(
    state_dir: &Path,
    repo: &Path,
    modules: Vec<Module>,
) -> Result<(), DispatchError> {
    run_cmd::run_modules(state_dir, repo, modules).await
}
