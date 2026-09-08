//! Requirement level and the tool a gate needs on `PATH` -- verify.sh:25-45.

/// Whether a gate missing its required tool makes the whole run an environment fault
/// (`Required`) or is merely noted and skipped (`Advisory`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Requirement {
    Required,
    Advisory,
}

/// The external tool a gate needs on `PATH` before it can even attempt to run.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ProbeTool {
    Cargo,
    CargoFmt,
    CargoClippy,
    CargoDeny,
    CargoAudit,
    CargoLlvmCov,
    CargoMutants,
    Named(&'static str),
}
