//! System-capacity measurement + preflight (BLUEPRINT gap fix: "no crashing again"). Real
//! numbers come from a `CapacityProbe` port (`measurement.rs`) so the pure refusal logic in
//! `preflight.rs` can be driven by a fake in tests; `StdCapacityProbe` (`std_probe.rs`) is the
//! one real implementation, backed by OS-specific readers (`macos.rs` / `linux.rs`).

mod lanes;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod macos_parse;
mod measurement;
mod preflight;
mod std_probe;

pub use lanes::ram_lanes_from_available;
pub use measurement::{CapacityProbe, Measurement};
pub use preflight::{preflight, PreflightConfig};
pub use std_probe::StdCapacityProbe;
