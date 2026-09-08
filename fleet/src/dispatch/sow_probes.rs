//! Adapters wiring `fleet sow`'s ambiguity check to `fleet-scan`'s 4 probes (`fleet-scan`'s own
//! doc comment: "this crate performs no IO of its own" -- every fact arrives through a
//! caller-supplied port). No codebase index or research backend is wired into the CLI yet, so
//! `Codebase` answers conservatively (never blocks or lies about existence) and `Research`
//! reports a typed fault instead of fabricating a result -- `assess` treats a faulted probe
//! exactly like one that raised zero questions (see `fleet_scan::assess`'s doc). Memory recall
//! IS wired, via `memory_adapter::RealMemory` (backed by `fleet_memory::retrieve`).

use fleet_scan::{CodebasePort, ConcurrentRunner, EnvFault, ProbeJob, ProbeRun, ResearchPort};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// No codebase index is wired into `sow` yet; every symbol reports "not found" rather than
/// blocking `sow` on a walk of the working tree (see `walk.rs` for why a full walk is not free).
pub struct StubCodebase;
impl CodebasePort for StubCodebase {
    fn symbol_exists(&self, _name: &str) -> Result<bool, EnvFault> {
        Ok(false)
    }
}

/// No research backend is wired into `sow` yet.
pub struct StubResearch;
impl ResearchPort for StubResearch {
    fn search(&self, _query: &str) -> Result<Vec<String>, EnvFault> {
        Err(EnvFault::CredentialMissing("research backend not wired into `sow` yet".into()))
    }
}

/// Runs the 4 probe jobs sequentially in-process. `sow`'s probes are near-instant text/stub-port
/// checks, so this trades the real concurrency `ConcurrentRunner` allows for simplicity; it still
/// honors the trait's panic-isolation contract by catching each job's panic individually.
pub struct SequentialRunner;
impl ConcurrentRunner for SequentialRunner {
    fn run4<'a>(&self, jobs: [ProbeJob<'a>; 4]) -> [ProbeRun; 4] {
        jobs.map(|job| match catch_unwind(AssertUnwindSafe(job)) {
            Ok(outcome) => ProbeRun::Completed(outcome),
            Err(payload) => ProbeRun::Panicked(panic_msg(&payload)),
        })
    }
}

fn panic_msg(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "probe panicked with non-string payload".into())
}
