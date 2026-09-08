//! Deterministic offline coverage of the pure core, using a fake `JudgeModel`. No network.
//! Runs under `cargo test -p fleet-judge` with no features. Split into `tests/offline/*` to
//! respect the crate's 80-line-per-file rule; `#[path]` is needed because a test-target crate
//! root's submodules live beside it, not under a same-named directory.

#[path = "offline/support.rs"]
mod support;
#[path = "offline/decisions.rs"]
mod decisions;
#[path = "offline/rejections.rs"]
mod rejections;
#[path = "offline/transport.rs"]
mod transport;
