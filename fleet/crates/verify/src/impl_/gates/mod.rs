//! Location-independent resolution of gate script assets.
//!
//! `GateSpec::command`'s script variants no longer carry `bin/...`-style paths that only resolve
//! when the process's cwd happens to be a fleet checkout. Instead every gate script this crate
//! invokes is embedded into the binary at compile time (`embed.rs`) and, by default, written out
//! to a fresh temp directory the first time it is needed (`materialize.rs`) -- with the
//! executable bit set -- so an installed binary run from any cwd can still find and run them. A
//! caller may instead supply a gates-root override (see `GatesRoot::from_override`) to use real
//! files on disk in place of the embedded copies, mirroring this crate's existing injected-port
//! style (`ports::ToolProbe`/`ports::ProcessRunner`) of letting the caller supply real IO.
//!
//! See `crates/fleet-verify/gates/` for what is embedded and the per-script relocation notes
//! (several of the wrapped scripts assume they sit inside a live fleet/ checkout for things this
//! crate does not and should not try to fake -- a `git` working tree, `memory/lessons/`, or the
//! rest of the source tree to scan; those notes explain exactly which gates need a real-checkout
//! override to behave as originally intended, versus which are fully self-contained).

mod embed;
mod error;
mod materialize;
mod root;

pub use error::GateAssetError;
pub use root::GatesRoot;
