//! The freelane keyless-lane asset (script + lane config), embedded and materialized the same
//! way `crates/fleet-verify/src/gates/` embeds its gate scripts, plus the logic to invoke it and
//! interpret its output. Restores what `bin/freelane.sh` used to do before a cleanup pass deleted
//! `fleet/bin/` without migrating it (it belongs to this crate, not `fleet-verify`). Ported in
//! spirit from keel's `run_freelane_agent`/`interpret_freelane_output`
//! (`git show HEAD:fleet/keel/fleet/src/main.rs:3261`).
//!
//! `src/dispatch/agent_cmd_run.rs` is the only caller: it calls `run` and maps the result onto
//! its own `AgentOutcome`.

mod apply;
mod embed;
mod error;
mod materialize;
mod output;
mod root;
mod run;

pub use apply::ApplyError;
pub use error::FreelaneAssetError;
pub use output::FreelaneOutput;
pub use root::FreelaneRoot;
pub use run::{run, FreelaneRunError};
