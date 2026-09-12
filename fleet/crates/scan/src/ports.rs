//! Ports this crate needs from its siblings (named logical edges, no Cargo deps -- see header).

use crate::fault::EnvFault;

/// The subset of `fleet-context`'s eventual API this crate needs: "does this identifier already
/// exist in the current repo under some name" -- `TechnicalProbe` uses it to ask whether a
/// requirement's named thing is genuinely new or is ambiguously re-describing existing code.
pub trait CodebasePort: Send + Sync {
    fn symbol_exists(&self, name: &str) -> Result<bool, EnvFault>;
}

/// The subset of `fleet-memory`'s eventual API this crate needs: recall prior requirements/
/// decisions textually similar to this one, so `MemoryProbe` can ask "this looks like it
/// contradicts/duplicates a past decision -- which one governs?".
pub trait MemoryPort: Send + Sync {
    fn recall_similar(&self, query: &str, limit: usize) -> Result<Vec<MemoryHit>, EnvFault>;
}

/// One remembered item similar to the current requirement text.
#[derive(Clone, Debug, PartialEq)]
pub struct MemoryHit {
    pub text: String,
    /// Similarity score as reported by `fleet-memory`'s own ranking; opaque to this crate beyond
    /// "higher is more similar" -- never compared for exact equality, only used to pick top-N.
    pub score: f32,
}

/// The subset of external research (web/docs lookup) `ResearchProbe` needs. This is the port most
/// likely to fault (§2) -- no network, no API key, no result found -- and its trait signature
/// reflects that every call is fallible in the ordinary case, not the exceptional one.
pub trait ResearchPort: Send + Sync {
    fn search(&self, query: &str) -> Result<Vec<String>, EnvFault>;
}

/// One probe invocation, boxed so `ConcurrentRunner` doesn't need to know each probe's concrete
/// type -- only that it is a `Send` closure producing a `ProbeOutcome` when run.
pub type ProbeJob<'a> = Box<dyn FnOnce() -> crate::probe::ProbeOutcome + Send + 'a>;

/// The result of running one `ProbeJob`: either it completed (with questions or a fault it
/// reported itself), or the runner caught a panic escaping it.
#[derive(Clone, Debug, PartialEq)]
pub enum ProbeRun {
    Completed(crate::probe::ProbeOutcome),
    Panicked(String),
}

/// Runs the 4 probe jobs concurrently and returns their outcomes in the fixed
/// business/technical/memory/research order, regardless of completion order. Implemented by
/// (eventually) `fleet-worker`; this crate never spawns anything itself. An implementation MUST
/// isolate a panic in one job into `ProbeRun::Panicked` for that job alone -- it must never let a
/// panic in one job prevent the other 3 jobs' results from being returned.
pub trait ConcurrentRunner: Send + Sync {
    fn run4<'a>(&self, jobs: [ProbeJob<'a>; 4]) -> [ProbeRun; 4];
}
