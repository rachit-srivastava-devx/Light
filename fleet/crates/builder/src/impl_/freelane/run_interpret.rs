//! Turns a finished `freelane.sh` child's raw `Output` into `(log, response)` or a typed
//! refusal -- split out of `run.rs` for size. See that file's own doc comment for the exit-code
//! contract this decodes.

use std::process::Output;

use super::FreelaneRunError;

pub(super) fn interpret(output: Output) -> Result<(String, String), FreelaneRunError> {
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
        None => {
            return Err(FreelaneRunError(
                "freelane: worker terminated without an exit code".into(),
            ))
        }
    }
    let response = String::from_utf8(output.stdout)
        .map_err(|_| FreelaneRunError("freelane: response was not UTF-8".into()))?;
    if response.trim().is_empty() {
        return Err(FreelaneRunError(
            "freelane: successful invocation returned an empty response".into(),
        ));
    }
    Ok((log, response))
}
