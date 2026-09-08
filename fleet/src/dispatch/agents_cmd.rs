//! `fleet agents`: parse -> `fleet_worker::resolve_hermetic_provision` -> print. **Deviation**:
//! the blueprint's reuse map cites `agent_registry::load_agent`/`manifest::manifest_for_lease`
//! directly, but `fleet-worker`'s `sandbox` module is private (`mod sandbox;`, not `pub mod`) --
//! only `resolve_hermetic_provision` (its whole-provision entry point) is re-exported from
//! `lib.rs`. This wires to that real, public entry point instead; flagged for Opus in case the
//! crate should re-export the narrower functions too.

use crate::cli::args_core::AgentsArgs;
use crate::dispatch::error::DispatchError;
use crate::print::human;
use fleet_worker::resolve_hermetic_provision;
use std::path::Path;

pub fn agents(args: AgentsArgs) -> Result<(), DispatchError> {
    let provision = resolve_hermetic_provision(Path::new(&args.repo), &args.agent_id)?;
    human::line("skills", format!("{:?}", provision.skill_ids));
    human::line("system_prompt", provision.system_prompt);
    Ok(())
}
