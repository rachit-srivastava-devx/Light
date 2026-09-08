//! Compile-time embedding of `crates/fleet-verify/gates/` -- baked into the binary so nothing at
//! run time resolves these paths via cwd or `$CARGO_MANIFEST_DIR` (that env var is a build-time
//! fact only; an installed binary has no such directory at run time).
//!
//! The four scripts this crate names directly in `registry.rs` are embedded individually via
//! `include_str!`, verbatim. `policy/` and `corpus/` are large fixture/detector trees (11 and
//! 114+ files respectively) embedded whole via `include_dir!` rather than listed file-by-file.

use include_dir::{include_dir, Dir};

pub(super) const SEMGREP_GATE_SH: &str = include_str!("../../gates/semgrep-gate.sh");
pub(super) const TRIVY_GATE_SH: &str = include_str!("../../gates/trivy-gate.sh");
pub(super) const RECUR_GATE_SH: &str = include_str!("../../gates/recur-gate.sh");
pub(super) const DETECTOR_INTEGRITY_SH: &str = include_str!("../../gates/detector-integrity.sh");

pub(super) static POLICY_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/gates/policy");
pub(super) static CORPUS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/gates/corpus");
