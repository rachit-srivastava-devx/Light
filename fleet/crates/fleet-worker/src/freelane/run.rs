//! Resolves the freelane asset root, invokes `freelane.sh`, and turns its exit code +
//! stdout/stderr into a `FreelaneOutput` or a typed refusal reason. Ported in spirit from keel's
//! `run_freelane_agent` (`git show HEAD:fleet/keel/fleet/src/main.rs:3350`).

use std::path::Path;
use std::process::{Command, Stdio};

use super::apply;
use super::error::FreelaneAssetError;
use super::output::{parse_resolved_model, parse_tokens, FreelaneOutput};
use super::root::FreelaneRoot;

/// freelane.sh's own exit codes (see `crates/fleet-worker/assets/freelane.sh`): 0 ok, 3 every
/// configured lane unavailable, 7 a usage error (missing/empty prompt). Any other code, an asset
/// resolution failure, or a launch failure is folded into the same typed refusal here -- an
/// honest reason string, never a fabricated "done".
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct FreelaneRunError(String);

impl From<FreelaneAssetError> for FreelaneRunError {
    fn from(err: FreelaneAssetError) -> Self {
        Self(format!("freelane: {err}"))
    }
}

/// `$FLEET_FREELANE_ROOT`, if set, overrides the embedded `freelane.sh` copy with real files on
/// disk (mirrors `$FLEET_GATES_ROOT` at `src/dispatch/verify_ports.rs`); otherwise the script
/// baked into the binary at compile time is materialized fresh, so this works regardless of the
/// caller's cwd. Then runs `<script> [--model M] <task>` with `cwd = worktree`, matching keel's
/// `Command::new(&script).arg(task).current_dir(repo)` plus freelane.sh's own `--model` flag.
pub fn run(worktree: &Path, task: &str, model: Option<&str>) -> Result<FreelaneOutput, FreelaneRunError> {
    let root = match std::env::var_os("FLEET_FREELANE_ROOT") {
        Some(path) => FreelaneRoot::from_override(path)?,
        None => FreelaneRoot::materialize()?,
    };
    let script = root.script()?;

    let mut cmd = Command::new(&script);
    if let Some(m) = model {
        cmd.arg("--model").arg(m);
    }
    let output = cmd
        .arg(task)
        .current_dir(worktree)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| FreelaneRunError(format!("freelane: cannot launch keyless lane: {e}")))?;

    let log = String::from_utf8_lossy(&output.stderr).trim().to_string();
    match output.status.code() {
        Some(0) => {}
        Some(code) => {
            return Err(FreelaneRunError(if log.is_empty() {
                format!("freelane: worker exited {code}")
            } else {
                log
            }))
        }
        None => return Err(FreelaneRunError("freelane: worker terminated without an exit code".into())),
    }
    let response = String::from_utf8(output.stdout)
        .map_err(|_| FreelaneRunError("freelane: response was not UTF-8".into()))?;
    if response.trim().is_empty() {
        return Err(FreelaneRunError(
            "freelane: successful invocation returned an empty response".into(),
        ));
    }
    let (applied_files, apply_note) = apply::try_apply(worktree, &response);
    Ok(FreelaneOutput {
        resolved_model: parse_resolved_model(&log),
        tokens: parse_tokens(&log),
        response,
        log,
        applied_files,
        apply_note,
    })
}
