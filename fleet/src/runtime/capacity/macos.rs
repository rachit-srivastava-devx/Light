//! macOS `CapacityProbe` backend: shells out to the same `sysctl`/`vm_stat` commands an operator
//! would run by hand, so every number here is cross-checkable on the command line (see the
//! task's step 6). Parsing lives in `macos_parse.rs` to keep this file ≤80 lines.

use super::macos_parse::{parse_loadavg, parse_u64, parse_vm_stat_available};
use super::measurement::{Measurement, ProbeError};
use std::process::Command;

pub fn measure(logical_cores: usize) -> Result<Measurement, ProbeError> {
    let total_memory_bytes = parse_u64(&run("sysctl", &["-n", "hw.memsize"])?, "sysctl hw.memsize")?;
    let available_memory_bytes = parse_vm_stat_available(&run("vm_stat", &[])?)?;
    let load_avg_1m = parse_loadavg(&run("sysctl", &["-n", "vm.loadavg"])?)?;
    Ok(Measurement { total_memory_bytes, available_memory_bytes, load_avg_1m, logical_cores })
}

fn run(command: &'static str, args: &[&str]) -> Result<String, ProbeError> {
    let out = Command::new(command).args(args).output().map_err(|e| ProbeError::SourceUnavailable {
        resource: command,
        detail: e.to_string(),
    })?;
    if !out.status.success() {
        return Err(ProbeError::SourceUnavailable {
            resource: command,
            detail: format!("exit status {}: {}", out.status, String::from_utf8_lossy(&out.stderr)),
        });
    }
    String::from_utf8(out.stdout)
        .map_err(|e| ProbeError::SourceUnavailable { resource: command, detail: e.to_string() })
}
