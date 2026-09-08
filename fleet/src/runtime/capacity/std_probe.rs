//! `StdCapacityProbe`: the one real `CapacityProbe` implementation, dispatched by OS. macOS
//! shells out to `sysctl`/`vm_stat` (`macos.rs`); Linux reads `/proc/meminfo`+`/proc/loadavg`
//! (`linux.rs`). Any other platform is a typed refusal, never a guess.

use super::measurement::{CapacityProbe, Measurement, ProbeError};
use std::num::NonZeroUsize;

pub struct StdCapacityProbe;

impl CapacityProbe for StdCapacityProbe {
    fn measure(&self) -> Result<Measurement, ProbeError> {
        let cores = std::thread::available_parallelism().map(NonZeroUsize::get).unwrap_or(1);
        measure_for_platform(cores)
    }
}

#[cfg(target_os = "macos")]
fn measure_for_platform(cores: usize) -> Result<Measurement, ProbeError> {
    super::macos::measure(cores)
}

#[cfg(target_os = "linux")]
fn measure_for_platform(cores: usize) -> Result<Measurement, ProbeError> {
    super::linux::measure(cores)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn measure_for_platform(_cores: usize) -> Result<Measurement, ProbeError> {
    Err(ProbeError::UnsupportedPlatform(std::env::consts::OS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_probe_measures_something_plausible_on_this_machine() {
        let m = StdCapacityProbe.measure().expect("probe works on macOS/Linux dev & CI machines");
        assert!(m.logical_cores >= 1);
        assert!(m.total_memory_bytes > 0);
        assert!(m.available_memory_bytes <= m.total_memory_bytes);
    }
}
