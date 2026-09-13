//! `probe-business`: requirement-ambiguity probe with an injected `BusinessReader` port.

mod port;
mod probe;

pub use port::{BusinessReader, ProbeError};
pub use probe::{probe, BusinessInput, Question};
