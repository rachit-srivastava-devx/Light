//! Shared test doubles + fixtures for `fleet-lifecycle`'s integration tests. Each test
//! binary only uses a subset of these, so unused-item lints are expected and suppressed here.
#![allow(dead_code, unused_imports)]

pub mod doubles;
pub mod fixtures;
pub mod variant;

pub use doubles::{FailingEmitter, FailingLedger, MemoryLedger, RecordingEmitter};
pub use fixtures::{accepted_task, complete_elements, sample_request};
pub use variant::variant_name;
