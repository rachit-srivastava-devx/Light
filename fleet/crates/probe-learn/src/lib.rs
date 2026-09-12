//! `probe-learn`: scoped learning probe with an injected `MemoryReader` port.

mod port;
mod probe;

pub use port::{LessonHit, MemoryReader, ProbeError};
pub use probe::{LearnInput, Question, probe};
