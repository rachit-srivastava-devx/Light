//! Read-only requirement-ambiguity probing: run 4 independent probes concurrently, merge and
//! dedup whatever questions they raise, and return an ADHD-safe verdict -- Clear, or at most 4
//! ranked questions. No probe may block the other 3: a probe that cannot complete (no network, no
//! credential, a caught panic) reports a typed `EnvFault` instead, and the merge proceeds with
//! whatever the other probes returned. This crate performs no IO of its own -- every fact about
//! the outside world (codebase lookups, memory recall, external research, and the concurrency to
//! run probes in parallel) arrives through a caller-supplied port trait.

mod assess;
mod business;
mod fault;
mod input;
mod jaccard;
mod memory;
mod merge;
mod open_questions;
mod ports;
mod probe;
mod research;
mod technical;

pub use assess::{assess, AssessmentReport, ProbeSet};
pub use business::BusinessProbe;
pub use fault::EnvFault;
pub use input::{ProbeKind, RequirementInput};
pub use jaccard::jaccard_similarity;
pub use memory::MemoryProbe;
pub use merge::merge_questions;
pub use open_questions::{Assessment, OpenQuestions, OpenQuestionsError};
pub use ports::{CodebasePort, ConcurrentRunner, MemoryHit, MemoryPort, ProbeJob, ProbeRun, ResearchPort};
pub use probe::{GapSeverity, Probe, ProbeOutcome, Question};
pub use research::ResearchProbe;
pub use technical::TechnicalProbe;
