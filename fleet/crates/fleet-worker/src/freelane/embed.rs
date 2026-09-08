//! Compile-time embedding of `crates/fleet-worker/assets/freelane.sh` (+ its default
//! `lanes.conf`) -- baked into the binary so nothing at run time resolves these paths via cwd or
//! `$CARGO_MANIFEST_DIR` (that env var is a build-time fact only; an installed binary has no such
//! directory at run time -- see `crates/fleet-verify/src/gates/embed.rs` for the same lesson
//! applied to the gate scripts, which this module mirrors).

pub(super) const FREELANE_SH: &str = include_str!("../../assets/freelane.sh");
pub(super) const LANES_CONF: &str = include_str!("../../assets/lanes.conf");
