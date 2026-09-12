//! `probe-research`: requirement-ambiguity probe with an injected `ResearchPort`.

mod port;
mod probe;

pub use port::{ResearchError, ResearchPort, Source};
pub use probe::{ProbeResult, Question, ResearchInput, probe};
