//! The facts a `CapacityProbe` reports, and the port itself -- injected so the pure preflight
//! logic in `preflight.rs` can be driven by a fake probe in tests instead of the real machine.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measurement {
    pub total_memory_bytes: u64,
    pub available_memory_bytes: u64,
    pub load_avg_1m: f64,
    pub logical_cores: usize,
}

/// A probe that cannot measure must say so -- never a silent zero, never a silent "assume
/// plenty" (BLUEPRINT: unknown capacity is not permission to proceed).
#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error("could not read {resource}: {detail}")]
    SourceUnavailable { resource: &'static str, detail: String },
    #[error("could not parse {field} from {resource}: {detail}")]
    ParseFailed { resource: &'static str, field: &'static str, detail: String },
    /// Only constructed on platforms other than macOS/Linux -- `dead_code` on the two platforms
    /// this repo actually builds for is expected, not a bug.
    #[allow(dead_code)]
    #[error("no capacity probe implemented for this platform ({0})")]
    UnsupportedPlatform(&'static str),
}

/// A port: reports raw facts about the machine. Real system access lives only in
/// `StdCapacityProbe`; every other type in this module takes the facts as data.
pub trait CapacityProbe {
    fn measure(&self) -> Result<Measurement, ProbeError>;
}
